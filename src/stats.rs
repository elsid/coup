use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use crate::game::{ALL_CARDS, Card, Settings};
use crate::run::{ALL_BOT_TYPES, BotType, run_game_with_bots};

#[derive(Default, Clone)]
pub struct Stats {
    games: usize,
    steps: Vec<usize>,
    turns: Vec<usize>,
    rounds: Vec<usize>,
    winner_bot_type: Vec<BotType>,
    winner_initial_cards: Vec<Vec<Card>>,
    winner_bot_type_and_initial_cards: Vec<(BotType, Vec<Card>)>,
}

pub fn collect_random_games_stats(seed: u64, number: usize, workers: usize, bot_types: Vec<BotType>, settings: Settings) -> Stats {
    let rng = Arc::new(Mutex::new(StdRng::seed_from_u64(seed)));
    let stats = Arc::new(Mutex::new(Stats::default()));
    let threads = (0..workers)
        .map(|_| {
            let worker_stats = stats.clone();
            let worker_rng = rng.clone();
            let worker_settings = settings.clone();
            let worker_bot_types = bot_types.clone();
            std::thread::spawn(move || {
                loop {
                    {
                        let mut locked_stats = worker_stats.lock().unwrap();
                        if locked_stats.games >= number {
                            break;
                        }
                        locked_stats.games += 1;
                    }
                    let seed = worker_rng.lock().unwrap().gen::<u64>();
                    let result = run_game_with_bots(seed, &worker_bot_types, worker_settings.clone(), false, None);
                    let mut locked_stats = worker_stats.lock().unwrap();
                    locked_stats.steps.push(result.end.step());
                    locked_stats.turns.push(result.end.turn());
                    locked_stats.rounds.push(result.end.round());
                    let winner = result.end.get_winner().unwrap();
                    locked_stats.winner_bot_type.push(worker_bot_types[winner]);
                    let cards: Vec<Card> = result.begin.get_player_view(winner).cards.iter().map(|v| v.kind).collect();
                    locked_stats.winner_initial_cards.push(cards.clone());
                    locked_stats.winner_bot_type_and_initial_cards.push((worker_bot_types[winner], cards));
                }
            })
        })
        .collect::<Vec<_>>();
    for thread in threads {
        thread.join().unwrap();
    }
    let result: Stats = stats.lock().unwrap().clone();
    result
}

pub fn print_stats(stats: &Stats) {
    let steps = count(&stats.steps);
    println!("steps: {}", steps.len());
    for (steps, games) in steps.iter() {
        println!("{} {}", steps, games);
    }
    println!();
    let turns = count(&stats.turns);
    println!("turns: {}", turns.len());
    for (turns, games) in turns.iter() {
        println!("{} {}", turns, games);
    }
    println!();
    let rounds = count(&stats.rounds);
    println!("rounds: {}", rounds.len());
    for (rounds, games) in rounds.iter() {
        println!("{} {}", rounds, games);
    }
    println!();
    let mut existing_winner_bot_type: HashMap<BotType, usize> = HashMap::new();
    for bot_type in stats.winner_bot_type.iter() {
        *existing_winner_bot_type.entry(*bot_type).or_insert(0) += 1;
    }
    let mut existing_winner_initial_cards: HashMap<Vec<Card>, usize> = HashMap::new();
    for cards in stats.winner_initial_cards.iter() {
        let mut cards = cards.clone();
        cards.sort();
        *existing_winner_initial_cards.entry(cards).or_insert(0) += 1;
    }
    let mut existing_winner_bot_type_and_initial_cards: HashMap<(BotType, Vec<Card>), usize> = HashMap::new();
    for (bot_type, cards) in stats.winner_bot_type_and_initial_cards.iter() {
        let mut cards = cards.clone();
        cards.sort();
        *existing_winner_bot_type_and_initial_cards.entry((*bot_type, cards)).or_insert(0) += 1;
    }
    let mut winner_bot_type: Vec<(BotType, usize)> = existing_winner_bot_type.into_iter()
        .map(|(k, v)| (k, v))
        .collect();
    winner_bot_type.sort_by_key(|(_, games)| *games);
    let mut winner_initial_cards: Vec<(Vec<Card>, usize)> = Vec::new();
    let mut winner_bot_type_and_initial_cards: Vec<((BotType, Vec<Card>), usize)> = Vec::new();
    for i in 0..ALL_CARDS.len() {
        for j in i..ALL_CARDS.len() {
            let cards = vec![ALL_CARDS[i], ALL_CARDS[j]];
            winner_initial_cards.push((
                cards.clone(),
                existing_winner_initial_cards.get(&cards).cloned().unwrap_or(0),
            ));
            for bot_type in ALL_BOT_TYPES.iter() {
                winner_bot_type_and_initial_cards.push((
                    (*bot_type, cards.clone()),
                    existing_winner_bot_type_and_initial_cards.get(&(*bot_type, cards.clone())).cloned().unwrap_or(0),
                ));
            }
        }
    }
    winner_initial_cards.sort_by_key(|(_, games)| *games);
    winner_bot_type_and_initial_cards.sort_by_key(|(_, games)| *games);
    println!("winner bot type");
    for (bot_type, games) in winner_bot_type.iter() {
        println!("{:?} {} {}%", bot_type, games, *games as f64 / stats.games as f64 * 100.0);
    }
    println!();
    println!("winner initial cards:");
    for (cards, games) in winner_initial_cards.iter() {
        println!("{:?} {} {}%", cards, games, *games as f64 / stats.games as f64 * 100.0);
    }
    println!();
    println!("winner bot type and initial cards");
    for ((bot_type, cards), games) in winner_bot_type_and_initial_cards.iter() {
        println!("{:?} {:?} {} {}%", bot_type, cards, games, *games as f64 / stats.games as f64 * 100.0);
    }
    println!();
}

fn count(values: &Vec<usize>) -> BTreeMap<usize, usize> {
    let mut result: BTreeMap<usize, usize> = BTreeMap::new();
    for value in values.iter() {
        *result.entry(*value).or_insert(0) += 1;
    }
    result
}
