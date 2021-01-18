use std::fs::File;
use std::io::{BufRead, BufReader};

use clap::Clap;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use crate::bots::{ActionView, CardsTracker, RandomBot, HonestCarefulRandomBot, Bot};
use crate::game::{Action, Blocker, Card, Game, get_example_actions, get_example_settings, OpponentView, PlayerCard, PlayerView, Settings, get_available_actions};
use crate::run::{BotType, run_game_with_bots};
use crate::stats::{collect_random_games_stats, print_stats};

mod game;
mod bots;
mod stats;
mod run;

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

fn main() {
    let args: Args = Args::parse();
    match args.command {
        Command::Simulate(params) => simulate(params),
        Command::Replay(params) => replay(params),
        Command::Stats(params) => stats(params),
        Command::Example => example(),
        Command::Track(params) => track(params),
        Command::Suggest(params) => suggest(params),
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
        let hand: Vec<Card> = view.cards.iter().map(|v| v.kind).collect();
        let mut tracker = CardsTracker::new(view.player, &hand, &settings);
        let mut step = 0;
        tracker.print();
        while let Some(action) = read_action(&mut file) {
            if let Some(view) = read_game_view(&mut file) {
                println!("[{}] play {:?} {:?}", step, action, view);
                if view.player == action.player {
                    tracker.after_player_action(&view.player_view(), &action);
                } else {
                    tracker.after_opponent_action(&view.player_view(), &ActionView::from_action(&action));
                }
                tracker.print();
                step += 1;
            } else {
                break;
            }
        }
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
        let player_view = view.player_view();
        match bot_type {
            BotType::Random => suggest_from_file_with_bot(&player_view, file, RandomBot::new(&player_view)),
            BotType::HonestCarefulRandom => suggest_from_file_with_bot(&player_view, file, HonestCarefulRandomBot::new(&player_view, &settings)),
        }
    }
}

fn suggest_from_file_with_bot<F: BufRead, B: Bot>(initial_view: &PlayerView, mut file: F, mut bot: B) {
    let available_actions = get_available_actions(initial_view.game_player, &initial_view.players, initial_view.blockers);
    let mut suggested_actions: Vec<Action> = bot.suggest_actions(initial_view, &available_actions).iter()
        .map(|v| (*v).clone())
        .collect();
    while let Some(action) = read_action(&mut file) {
        if let Some(view) = read_game_view(&mut file) {
            if view.player == action.player {
                bot.after_player_action(&view.player_view(), &action);
            } else {
                bot.after_opponent_action(&view.player_view(), &ActionView::from_action(&action));
            }
            let available_actions = get_available_actions(initial_view.game_player, &initial_view.players, initial_view.blockers);
            suggested_actions = bot.suggest_actions(initial_view, &available_actions).iter()
                .map(|v| (*v).clone())
                .collect();
        } else {
            break;
        }
    }
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
    game_player: usize,
    player: usize,
    coins: usize,
    cards: Vec<PlayerCard>,
    players: Vec<OpponentView>,
    blockers: Vec<Blocker>,
}

impl GameView {
    fn player_view(&self) -> PlayerView {
        PlayerView {
            step: self.step,
            turn: self.turn,
            round: self.round,
            game_player: self.game_player,
            player: self.player,
            coins: self.coins,
            cards: &self.cards,
            players: self.players.clone(),
            blockers: &self.blockers,
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
