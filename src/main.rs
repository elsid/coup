#[macro_use]
extern crate scan_fmt;

use std::fs::File;
use std::io::{BufRead, BufReader};

use clap::Clap;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::bots::{ActionView, Bot, CardsTracker, HonestCarefulRandomBot, is_allowed_action_type, RandomBot};
use crate::fsm::{Action, Card, StateType};
use crate::game::{Game, get_available_actions, get_example_actions, get_example_settings, PlayerView, Settings};
use crate::interactive::run_interactive_game;
use crate::run::{BotType, run_game_with_bots};
use crate::stats::{collect_random_games_stats, print_stats};

mod game;
mod bots;
mod stats;
mod run;
mod fsm;
mod interactive;

#[derive(Clap)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clap)]
enum Command {
    Simulate(SimulateParams),
    Replay(ReplayParams),
    Stats(StatsParams),
    Example,
    Track(TrackerParams),
    Suggest(SuggestParams),
    Fuzzy(FuzzyParams),
    Interactive,
}

#[derive(Clap, Debug)]
struct SimulateParams {
    #[clap(long)]
    bot_types: Vec<BotType>,
    #[clap(long, default_value = "42")]
    seed: u64,
    #[clap(long, default_value = "0")]
    max_steps: usize,
    #[clap(long, default_value = "6")]
    players_number: usize,
    #[clap(long, default_value = "3")]
    cards_per_type: usize,
    #[clap(long)]
    write_player: Option<usize>,
}

#[derive(Clap)]
struct StatsParams {
    #[clap(long, default_value = "100000")]
    games: usize,
    #[clap(long, default_value = "1")]
    workers: usize,
    #[clap(long, default_value = "42")]
    seed: u64,
    #[clap(long)]
    bot_types: Vec<BotType>,
    #[clap(long, default_value = "6")]
    players_number: usize,
    #[clap(long, default_value = "3")]
    cards_per_type: usize,
}

#[derive(Clap)]
struct ReplayParams {
    #[clap(long)]
    verbose: bool,
    #[clap(long)]
    write_player: Option<usize>,
    file: Option<String>,
}

#[derive(Clap)]
struct TrackerParams {
    file: Option<String>,
}

#[derive(Clap)]
struct SuggestParams {
    #[clap(long)]
    bot_type: BotType,
    file: Option<String>,
}

#[derive(Clap, Debug)]
struct FuzzyParams {
    #[clap(long, default_value = "42")]
    seed: u64,
    #[clap(long, default_value = "10000")]
    max_games: usize,
    #[clap(long, default_value = "6")]
    players_number: usize,
    #[clap(long, default_value = "3")]
    cards_per_type: usize,
}

fn main() {
    let args: Args = Args::parse();
    match args.command {
        Command::Simulate(params) => simulate(params),
        Command::Replay(params) => replay(params),
        Command::Stats(params) => stats(params),
        Command::Example => example(),
        Command::Track(params) => track(params),
        Command::Suggest(params) => suggest(params),
        Command::Fuzzy(params) => fuzzy(params),
        Command::Interactive => run_interactive_game(),
    }
}

fn simulate(params: SimulateParams) {
    let settings = Settings {
        players_number: params.players_number,
        cards_per_type: params.cards_per_type,
    };
    run_game_with_bots(params.seed, &params.bot_types, settings, true, params.write_player);
}

fn replay(params: ReplayParams) {
    if let Some(path) = params.file {
        replay_from_file(BufReader::new(File::open(path).unwrap()), params.verbose, params.write_player);
    } else {
        replay_from_file(std::io::stdin().lock(), params.verbose, params.write_player);
    }
}

#[derive(Serialize, Deserialize)]
struct GameParams {
    seed: u64,
    settings: Settings,
}

fn replay_from_file<F: BufRead>(mut file: F, verbose: bool, write_player: Option<usize>) {
    let mut line = String::new();
    file.read_line(&mut line).unwrap();
    let params: GameParams = serde_json::from_str(&line).unwrap();
    let mut rng = StdRng::seed_from_u64(params.seed);
    let mut game = Game::new(params.settings.clone(), &mut rng);
    if let Some(player) = write_player {
        println!("{}", serde_json::to_string(&params.settings).unwrap());
        println!("{}", serde_json::to_string(&game.get_player_view(player)).unwrap());
    }
    loop {
        let mut line = String::new();
        file.read_line(&mut line).unwrap();
        if line.is_empty() {
            break;
        }
        if verbose {
            game.print();
        }
        let action: Action = serde_json::from_str(&line).unwrap();
        if verbose {
            println!("[{}] play {:?}", game.step(), action);
        }
        if write_player.is_some() {
            println!("{}", serde_json::to_string(&action).unwrap());
        }
        assert_eq!(game.play(&action, &mut rng), Ok(()));
        if let Some(player) = write_player {
            println!("{}", serde_json::to_string(&game.get_player_view(player)).unwrap());
        }
    }
    if verbose {
        game.print();
    }
}

fn stats(params: StatsParams) {
    let settings = Settings {
        players_number: params.players_number,
        cards_per_type: params.cards_per_type,
    };
    print_stats(&collect_random_games_stats(params.seed, params.games, params.workers, params.bot_types, settings));
}

fn example() {
    let settings = get_example_settings();
    println!("{}", serde_json::to_string(&GameParams { seed: 42, settings }).unwrap());
    for action in get_example_actions() {
        println!("{}", serde_json::to_string(&action).unwrap());
    }
}

fn track(params: TrackerParams) {
    if let Some(path) = params.file {
        track_from_file(BufReader::new(File::open(path).unwrap()));
    } else {
        track_from_file(std::io::stdin().lock());
    }
}

fn track_from_file<F: BufRead>(mut file: F) {
    let mut line = String::new();
    file.read_line(&mut line).unwrap();
    let settings: Settings = serde_json::from_str(&line).unwrap();
    if let Some(view) = read_game_view(&mut file) {
        println!("[{}] View {:?}", view.step, view);
        let mut tracker = CardsTracker::new(view.player, &view.cards, &settings);
        while let Some(action) = read_action(&mut file) {
            println!("[{}] Play {:?}", view.step, action);
            if let Some(view) = read_game_view(&mut file) {
                println!("[{}] View {:?}", view.step, view);
                if view.player == action.player {
                    tracker.after_player_action(&view.player_view(), &action);
                } else {
                    tracker.after_opponent_action(&view.player_view(), &ActionView::from_action(&action));
                }
            } else {
                break;
            }
        }
        print!("[{}] Track ", view.step);
        tracker.print();
    }
}

fn suggest(params: SuggestParams) {
    if let Some(path) = params.file {
        suggest_from_file(params.bot_type, BufReader::new(File::open(path).unwrap()));
    } else {
        suggest_from_file(params.bot_type, std::io::stdin().lock());
    }
}

fn suggest_from_file<F: BufRead>(bot_type: BotType, mut file: F) {
    let mut line = String::new();
    file.read_line(&mut line).unwrap();
    let settings: Settings = serde_json::from_str(&line).unwrap();
    if let Some(view) = read_game_view(&mut file) {
        match bot_type {
            BotType::Random => {
                let bot = RandomBot::new(&view.player_view());
                suggest_from_file_with_bot(view, file, bot);
            }
            BotType::HonestCarefulRandom => {
                let bot = HonestCarefulRandomBot::new(&view.player_view(), &settings);
                suggest_from_file_with_bot(view, file, bot)
            }
        }
    }
}

fn suggest_from_file_with_bot<F: BufRead, B: Bot>(initial_view: GameView, mut file: F, mut bot: B) {
    let initial_player_view = initial_view.player_view();
    let available_actions: Vec<Action> = get_available_actions(initial_player_view.state_type, initial_player_view.player_coins, initial_player_view.player_hands).into_iter()
        .filter(|action| action.player == initial_view.player)
        .collect();
    let mut suggested_actions: Vec<Action> = bot.suggest_actions(&initial_player_view, &available_actions).iter()
        .map(|v| (*v).clone())
        .collect();
    let mut last_view = initial_view;
    while let Some(action) = read_action(&mut file) {
        if let Some(view) = read_game_view(&mut file) {
            if view.player == action.player {
                bot.after_player_action(&view.player_view(), &action);
            } else {
                bot.after_opponent_action(&view.player_view(), &ActionView::from_action(&action));
            }
            let available_actions: Vec<Action> = get_available_actions(&view.state_type, &view.player_coins, &view.player_hands).into_iter()
                .filter(|action| action.player == view.player)
                .collect();
            suggested_actions = bot.suggest_actions(&view.player_view(), &available_actions).iter()
                .map(|v| (*v).clone())
                .collect();
            last_view = view;
        } else {
            break;
        }
    }
    println!("[{}] {:?}", last_view.step, last_view);
    for action in suggested_actions {
        println!("{}", serde_json::to_string(&action).unwrap());
    }
}

fn read_action<F: BufRead>(file: &mut F) -> Option<Action> {
    let mut line = String::new();
    file.read_line(&mut line).unwrap();
    if line.is_empty() {
        return None;
    }
    Some(serde_json::from_str(&line).unwrap())
}

#[derive(Debug, Deserialize)]
struct GameView {
    step: usize,
    turn: usize,
    round: usize,
    player: usize,
    coins: usize,
    cards: Vec<Card>,
    state_type: StateType,
    player_coins: Vec<usize>,
    player_hands: Vec<usize>,
    player_cards: Vec<usize>,
    revealed_cards: Vec<Card>,
    deck: usize,
}

impl GameView {
    fn player_view(&self) -> PlayerView {
        PlayerView {
            step: self.step,
            turn: self.turn,
            round: self.round,
            player: self.player,
            coins: self.coins,
            cards: &self.cards,
            state_type: &self.state_type,
            player_coins: &self.player_coins,
            player_hands: &self.player_hands,
            player_cards: &self.player_cards,
            revealed_cards: &self.revealed_cards,
            deck: self.deck,
        }
    }
}

fn read_game_view<F: BufRead>(file: &mut F) -> Option<GameView> {
    let mut line = String::new();
    file.read_line(&mut line).unwrap();
    if line.is_empty() {
        return None;
    }
    Some(serde_json::from_str(&line).unwrap())
}

fn fuzzy(params: FuzzyParams) {
    let mut rng = StdRng::seed_from_u64(params.seed);
    let settings = Settings {
        players_number: params.players_number,
        cards_per_type: params.cards_per_type,
    };
    for _ in 0..params.max_games {
        let mut record: Vec<(Game, Action)> = Vec::new();
        let mut game = Game::new(settings.clone(), &mut rng);
        while !game.is_done() {
            let view = game.get_anonymous_view();
            let available_actions = get_available_actions(view.state_type, view.player_coins, view.player_hands);
            let mut allowed_actions: Vec<Action> = available_actions.iter()
                .filter(|action| is_allowed_action_type(&action.action_type, game.get_player_view(action.player).cards))
                .cloned()
                .collect();
            if allowed_actions.is_empty() {
                for (game, action) in record {
                    game.print();
                    println!("Play {:?}", action);
                }
                game.print();
                panic!("No allowed actions");
            }
            for action in available_actions {
                if is_allowed_action_type(&action.action_type, game.get_player_view(action.player).cards) {
                    continue;
                }
                if let Ok(()) = game.play(&action, &mut rng) {
                    panic!("Not allowed action is applied: {:?}", action);
                }
            }
            allowed_actions.shuffle(&mut rng);
            let mut errors: Vec<(Action, String)> = Vec::new();
            while let Some(action) = allowed_actions.pop() {
                let game_copy = game.clone();
                if let Err(e) = game.play(&action, &mut rng) {
                    errors.push((action, e));
                    if allowed_actions.is_empty() {
                        for (game, action) in record {
                            game.print();
                            println!("Play {:?}", action);
                        }
                        game.print();
                        for (action, error) in errors {
                            println!("{:?} {:?}", action, error);
                        }
                        panic!("All allowed actions are wrong");
                    }
                } else {
                    record.push((game_copy, action));
                    break;
                }
            }
        }
    }
}
