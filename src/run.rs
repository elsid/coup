use std::collections::BTreeMap;
use std::str::FromStr;

use itertools::Itertools;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use crate::bots::{ActionView, Bot, HonestCarefulRandomBot, RandomBot};
use crate::game::{Action, ActionType, Game, Settings, get_available_actions};

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
        let mut available_actions_per_player: BTreeMap<usize, Vec<Action>> = BTreeMap::new();
        let view = game.get_anonymous_view();
        let actions = get_available_actions(view.player, &view.players, view.blockers);
        for (player, group) in &actions.into_iter().group_by(|action| action.player) {
            available_actions_per_player.insert(player, group.collect());
        }
        let action = get_action(&available_actions_per_player, bots, game);
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

pub fn get_action<B: AsMut<dyn Bot>>(available_actions_per_player: &BTreeMap<usize, Vec<Action>>, bots: &mut [B], game: &Game) -> Action {
    if available_actions_per_player.len() > 1 {
        let (last_player, last_player_available_actions) = available_actions_per_player.iter()
            .find(|(_, available_actions)| {
                available_actions.iter()
                    .any(|action| matches!(action.action_type, ActionType::Complete))
            })
            .unwrap();
        for (player, available_actions) in available_actions_per_player.iter() {
            if *player > *last_player {
                if let Some(action) = bots[*player].as_mut().get_optional_action(&game.get_player_view(*player), available_actions) {
                    return action;
                }
            }
        }
        for (player, available_actions) in available_actions_per_player.iter() {
            if *player < *last_player {
                if let Some(action) = bots[*player].as_mut().get_optional_action(&game.get_player_view(*player), available_actions) {
                    return action;
                }
            }
        }
        bots[*last_player].as_mut().get_action(&game.get_player_view(*last_player), last_player_available_actions)
    } else {
        let (player, available_actions) = available_actions_per_player.iter().next().unwrap();
        bots[*player].as_mut().get_action(&game.get_player_view(*player), available_actions)
    }
}
