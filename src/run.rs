use std::str::FromStr;

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use crate::bots::{ActionView, Bot, HonestCarefulRandomBot, RandomBot};
use crate::fsm::Action;
use crate::game::{Game, get_available_actions, Settings};

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum BotType {
    Random,
    HonestCarefulRandom,
}

pub const ALL_BOT_TYPES: [BotType; 2] = [
    BotType::Random,
    BotType::HonestCarefulRandom,
];

impl FromStr for BotType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "random" => Ok(BotType::Random),
            "honest_careful_random" => Ok(BotType::HonestCarefulRandom),
            _ => Err(format!("invalid bot type: {}", s)),
        }
    }
}

pub struct RunResult {
    pub begin: Game,
    pub end: Game,
}

pub fn run_game_with_bots(seed: u64, bot_types: &[BotType], settings: Settings, verbose: bool, write_player: Option<usize>) -> RunResult {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut game = Game::new(settings.clone(), &mut rng);
    let begin = game.clone();
    let mut bots: Vec<Box<dyn Bot>> = bot_types.iter()
        .enumerate()
        .map(|(index, bot_type)| -> Box<dyn Bot> {
            match bot_type {
                BotType::Random => {
                    Box::new(RandomBot::new(&game.get_player_view(index)))
                }
                BotType::HonestCarefulRandom => {
                    Box::new(HonestCarefulRandomBot::new(&game.get_player_view(index), &settings))
                }
            }
        })
        .collect();
    run_game(&mut bots, &mut game, &mut rng, verbose, write_player);
    RunResult { begin, end: game }
}

pub fn run_game<B: AsMut<dyn Bot>, R: Rng>(bots: &mut [B], game: &mut Game, rng: &mut R, verbose: bool, write_player: Option<usize>) {
    if verbose {
        game.print();
    }
    if let Some(player) = write_player {
        println!("{}", serde_json::to_string(&game.get_player_view(player)).unwrap());
    }
    while !game.is_done() {
        let view = game.get_anonymous_view();
        let available_actions = get_available_actions(view.state_type, view.player_coins, view.player_hands);
        let action = get_action(&available_actions, bots, game);
        if verbose {
            println!("play {:?}", action);
        }
        assert_eq!(game.play(&action, rng), Ok(()));
        if verbose {
            game.print();
        }
        for player in 0..bots.len() {
            let view = game.get_player_view(player);
            if write_player == Some(player) {
                println!("{}", serde_json::to_string(&view).unwrap());
            }
            if game.is_player_active(player) {
                if player == action.player {
                    bots[player].as_mut().after_player_action(&view, &action);
                } else {
                    bots[player].as_mut().after_opponent_action(&view, &ActionView::from_action(&action));
                }
            }
        }
    }
}

pub fn get_action<B: AsMut<dyn Bot>>(available_actions: &Vec<Action>, bots: &mut [B], game: &Game) -> Action {
    let mut players = Vec::new();
    for action in available_actions.iter() {
        if !players.contains(&action.player) {
            players.push(action.player);
        }
    }
    if players.len() > 1 {
        for player in &players[0..players.len() - 1] {
            let player_available_actions: Vec<Action> = available_actions.iter()
                .filter(|action| action.player == *player)
                .cloned()
                .collect();
            if let Some(action) = bots[*player].as_mut().get_optional_action(&game.get_player_view(*player), &player_available_actions) {
                return action;
            }
        }
        let last_player = players[players.len() - 1];
        let last_player_available_actions: Vec<Action> = available_actions.iter()
            .filter(|action| action.player == last_player)
            .cloned()
            .collect();
        bots[last_player].as_mut().get_action(&game.get_player_view(last_player), &last_player_available_actions)
    } else {
        let player = players[0];
        bots[player].as_mut().get_action(&game.get_player_view(player), available_actions)
    }
}
