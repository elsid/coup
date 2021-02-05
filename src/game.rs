use itertools::Itertools;
use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::fsm::{Action, ActionType, ASSASSINATION_COST, Card, CARDS_PER_PLAYER, ChallengeState, COUP_COST, MAX_CARDS_TO_EXCHANGE, MAX_COINS, play_action, State, StateType};

pub const ALL_CARDS: [Card; 5] = [Card::Assassin, Card::Ambassador, Card::Captain, Card::Contessa, Card::Duke];
pub const INITIAL_COINS: usize = 2;

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct PlayerView<'a> {
    pub step: usize,
    pub turn: usize,
    pub round: usize,
    pub player: usize,
    pub coins: usize,
    pub cards: &'a [Card],
    pub state_type: &'a StateType,
    pub player_coins: &'a [usize],
    pub player_hands: &'a [usize],
    pub player_cards: &'a [usize],
    pub revealed_cards: &'a [Card],
    pub deck: usize,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct AnonymousView<'a> {
    pub step: usize,
    pub turn: usize,
    pub round: usize,
    pub state_type: &'a StateType,
    pub player_coins: &'a [usize],
    pub player_hands: &'a [usize],
    pub player_cards: &'a [usize],
    pub revealed_cards: &'a [Card],
    pub deck: usize,
}

pub fn get_available_actions(state_type: &StateType, player_coins: &[usize], player_hands: &[usize]) -> Vec<Action> {
    match state_type {
        StateType::Turn { player } => {
            get_turn_available_actions(*player, player_coins, player_hands)
        }
        StateType::ForeignAid { player } => {
            get_foreign_aid_available_actions(*player, player_hands)
        }
        StateType::Tax { player }
        | StateType::Exchange { player }
        | StateType::BlockForeignAid { player, .. }
        | StateType::BlockSteal { player, .. }
        | StateType::BlockAssassination { player, .. } => {
            get_non_blocking_available_actions(*player, player_hands)
        }
        StateType::Assassination { player, target, can_challenge } => {
            get_assassination_available_actions(*player, *target, *can_challenge, player_hands)
        }
        StateType::Steal { player, target, can_challenge } => {
            get_steal_available_actions(*player, *target, *can_challenge, player_hands)
        }
        StateType::Challenge { state, .. } => get_challenge_available_actions(state),
        StateType::NeedCards { player, .. } => get_need_cards_available_actions(*player),
        StateType::TookCards { player, .. }
        | StateType::DroppedCard { player, .. } => {
            get_drop_card_actions(*player)
        }
        StateType::LostInfluence { player, .. } => {
            get_lost_influence_available_actions(*player)
        }
    }
}

pub fn get_turn_available_actions(player: usize, player_coins: &[usize], player_hands: &[usize]) -> Vec<Action> {
    if player_coins[player] >= MAX_COINS {
        let mut actions: Vec<Action> = Vec::with_capacity(player_hands.len());
        for other_player in 0..player_hands.len() {
            if other_player != player && player_hands[other_player] > 0 {
                actions.push(Action { player, action_type: ActionType::Coup(other_player) });
            }
        }
        return actions;
    }
    let action_types = [ActionType::Income, ActionType::ForeignAid, ActionType::Tax, ActionType::Exchange];
    let mut actions: Vec<Action> = Vec::with_capacity(action_types.len() + 3 * (player_hands.len() - 1));
    for action_type in action_types.iter().cloned() {
        actions.push(Action { player, action_type });
    }
    for other_player in 0..player_hands.len() {
        if other_player != player && player_hands[other_player] > 0 {
            actions.push(Action { player, action_type: ActionType::Steal(other_player) });
            if player_coins[player] >= ASSASSINATION_COST {
                actions.push(Action { player, action_type: ActionType::Assassinate(other_player) });
            }
            if player_coins[player] >= COUP_COST {
                actions.push(Action { player, action_type: ActionType::Coup(other_player) });
            }
        }
    }
    actions
}

pub fn get_foreign_aid_available_actions(player: usize, player_hands: &[usize]) -> Vec<Action> {
    let mut actions: Vec<Action> = Vec::with_capacity(player_hands.len());
    fill_actions(&ActionType::BlockForeignAid, player, player_hands, &mut actions);
    actions.push(Action { player, action_type: ActionType::PassBlock });
    actions
}

pub fn get_non_blocking_available_actions(player: usize, player_hands: &[usize]) -> Vec<Action> {
    let mut actions: Vec<Action> = Vec::with_capacity(player_hands.len());
    fill_challenge_actions(player, player_hands, &mut actions);
    actions.push(Action { player, action_type: ActionType::PassChallenge });
    actions
}

pub fn get_assassination_available_actions(player: usize, target: usize, can_challenge: bool, player_hands: &[usize]) -> Vec<Action> {
    if can_challenge {
        let mut actions: Vec<Action> = Vec::with_capacity(player_hands.len());
        fill_challenge_actions(player, player_hands, &mut actions);
        actions.push(Action { player, action_type: ActionType::PassChallenge });
        actions
    } else {
        let mut actions = if player_hands[target] > 0 {
            let mut actions: Vec<Action> = Vec::with_capacity(2);
            actions.push(Action { player: target, action_type: ActionType::BlockAssassination });
            actions
        } else {
            Vec::with_capacity(1)
        };
        actions.push(Action { player, action_type: ActionType::PassBlock });
        actions
    }
}

pub fn get_steal_available_actions(player: usize, target: usize, can_challenge: bool, player_hands: &[usize]) -> Vec<Action> {
    if can_challenge {
        let mut actions: Vec<Action> = Vec::with_capacity(player_hands.len());
        fill_challenge_actions(player, player_hands, &mut actions);
        actions.push(Action { player, action_type: ActionType::PassChallenge });
        actions
    } else {
        let mut actions = if player_hands[target] > 0 {
            let mut actions: Vec<Action> = Vec::with_capacity(3);
            actions.push(Action { player: target, action_type: ActionType::BlockSteal(Card::Ambassador) });
            actions.push(Action { player: target, action_type: ActionType::BlockSteal(Card::Captain) });
            actions
        } else {
            Vec::with_capacity(1)
        };
        actions.push(Action { player, action_type: ActionType::PassBlock });
        actions
    }
}

pub fn get_challenge_available_actions(state: &ChallengeState) -> Vec<Action> {
    match state {
        ChallengeState::Initial { target, card, .. } => {
            let mut actions: Vec<Action> = Vec::with_capacity(ALL_CARDS.len() + 1);
            actions.push(Action {
                player: *target,
                action_type: ActionType::ShowCard(*card),
            });
            for other_card in &ALL_CARDS {
                actions.push(Action {
                    player: *target,
                    action_type: ActionType::RevealCard(*other_card),
                });
            }
            actions
        }
        ChallengeState::ShownCard { initiator, .. } => {
            let mut actions: Vec<Action> = Vec::with_capacity(ALL_CARDS.len());
            for card in &ALL_CARDS {
                actions.push(Action {
                    player: *initiator,
                    action_type: ActionType::RevealCard(*card),
                });
            }
            actions
        }
        ChallengeState::InitiatorRevealedCard { target } => {
            vec![Action { player: *target, action_type: ActionType::ShuffleDeck }]
        }
        ChallengeState::DeckShuffled { target } => {
            vec![Action { player: *target, action_type: ActionType::TakeCard }]
        }
        _ => Vec::new(),
    }
}

fn get_need_cards_available_actions(player: usize) -> Vec<Action> {
    vec![Action { player, action_type: ActionType::TakeCard }]
}

fn get_drop_card_actions(player: usize) -> Vec<Action> {
    let mut actions: Vec<Action> = Vec::with_capacity(ALL_CARDS.len());
    for card in &ALL_CARDS {
        actions.push(Action { player, action_type: ActionType::DropCard(*card) });
    }
    actions
}

fn get_lost_influence_available_actions(player: usize) -> Vec<Action> {
    let mut actions: Vec<Action> = Vec::with_capacity(ALL_CARDS.len());
    for card in &ALL_CARDS {
        actions.push(Action { player, action_type: ActionType::RevealCard(*card) });
    }
    actions
}

fn fill_challenge_actions(target: usize, player_hands: &[usize], actions: &mut Vec<Action>) {
    fill_actions(&ActionType::Challenge, target, player_hands, actions);
}

fn fill_actions(action_type: &ActionType, target: usize, player_hands: &[usize], actions: &mut Vec<Action>) {
    for player in target + 1..player_hands.len() {
        if player_hands[player] > 0 {
            actions.push(Action { player, action_type: action_type.clone() });
        }
    }
    for player in 0..target {
        if player_hands[player] > 0 {
            actions.push(Action { player, action_type: action_type.clone() });
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub players_number: usize,
    pub cards_per_type: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    step: usize,
    turn: usize,
    round: usize,
    player: usize,
    state_type: StateType,
    player_coins: Vec<usize>,
    player_hands: Vec<usize>,
    player_cards_counter: Vec<usize>,
    player_cards: Vec<Vec<Card>>,
    revealed_cards: Vec<Card>,
    deck: Vec<Card>,
}

pub fn make_deck(cards_per_type: usize) -> Vec<Card> {
    let mut deck = Vec::new();
    for card in &ALL_CARDS {
        for _ in 0..cards_per_type {
            deck.push(*card);
        }
    }
    deck
}

impl Game {
    pub fn new<R: Rng>(settings: Settings, rng: &mut R) -> Self {
        let mut deck = make_deck(settings.cards_per_type);
        deck.shuffle(rng);
        let deck_size = deck.len() - CARDS_PER_PLAYER * settings.players_number;
        let max_player_cards = CARDS_PER_PLAYER + MAX_CARDS_TO_EXCHANGE.min(deck_size);
        let mut player_cards: Vec<Vec<Card>> = (0..settings.players_number)
            .map(|_| Vec::with_capacity(max_player_cards))
            .take(settings.players_number)
            .collect();
        for _ in 0..CARDS_PER_PLAYER {
            for player_cards in player_cards.iter_mut() {
                player_cards.push(deck.pop().unwrap());
            }
        }
        for player_cards in player_cards.iter_mut() {
            player_cards.sort();
        }
        Self {
            step: 0,
            turn: 0,
            round: 0,
            player: 0,
            state_type: StateType::Turn { player: 0 },
            player_coins: std::iter::repeat(INITIAL_COINS).take(settings.players_number).collect(),
            player_hands: std::iter::repeat(CARDS_PER_PLAYER).take(settings.players_number).collect(),
            player_cards_counter: std::iter::repeat(CARDS_PER_PLAYER).take(settings.players_number).collect(),
            player_cards,
            revealed_cards: Vec::with_capacity(settings.cards_per_type * ALL_CARDS.len()),
            deck,
        }
    }

    #[cfg(test)]
    pub fn custom(mut player_cards: Vec<Vec<Card>>, deck: Vec<Card>) -> Self {
        for player_cards in player_cards.iter_mut() {
            player_cards.sort();
        }
        Self {
            step: 0,
            turn: 0,
            round: 0,
            player: 0,
            state_type: StateType::Turn { player: 0 },
            player_coins: std::iter::repeat(INITIAL_COINS).take(player_cards.len()).collect(),
            player_hands: std::iter::repeat(CARDS_PER_PLAYER).take(player_cards.len()).collect(),
            player_cards_counter: std::iter::repeat(CARDS_PER_PLAYER).take(player_cards.len()).collect(),
            revealed_cards: Vec::with_capacity(CARDS_PER_PLAYER * player_cards.len() + deck.len()),
            player_cards,
            deck,
        }
    }

    pub fn step(&self) -> usize {
        self.step
    }

    pub fn turn(&self) -> usize {
        self.turn
    }

    pub fn round(&self) -> usize {
        self.round
    }

    pub fn get_anonymous_view(&self) -> AnonymousView {
        AnonymousView {
            step: self.step,
            turn: self.turn,
            round: self.round,
            state_type: &self.state_type,
            player_coins: &self.player_coins,
            player_hands: &self.player_hands,
            player_cards: &self.player_cards_counter,
            revealed_cards: &self.revealed_cards,
            deck: self.deck.len(),
        }
    }

    pub fn get_player_view(&self, player: usize) -> PlayerView {
        PlayerView {
            step: self.step,
            turn: self.turn,
            round: self.round,
            player,
            coins: self.player_coins[player],
            cards: &self.player_cards[player],
            state_type: &self.state_type,
            player_coins: &self.player_coins,
            player_hands: &self.player_hands,
            player_cards: &self.player_cards_counter,
            revealed_cards: &self.revealed_cards,
            deck: self.deck.len(),
        }
    }

    pub fn is_player_active(&self, index: usize) -> bool {
        self.player_hands[index] > 0
    }

    pub fn is_done(&self) -> bool {
        self.player_hands.iter().filter(|hand| **hand > 0).count() <= 1
    }

    pub fn get_winner(&self) -> Option<usize> {
        if self.is_done() {
            self.player_hands.iter()
                .find_position(|hand| **hand > 0)
                .map(|(index, _)| index)
        } else {
            None
        }
    }

    pub fn play<R: Rng>(&mut self, action: &Action, rng: &mut R) -> Result<(), String> {
        let mut state = State {
            state_type: &mut self.state_type,
            player_coins: &mut self.player_coins,
            player_hands: &mut self.player_hands,
            player_cards_counter: &mut self.player_cards_counter,
            player_cards: &mut self.player_cards,
            deck: &mut self.deck,
            revealed_cards: &mut self.revealed_cards,
        };
        if let Err(e) = play_action(action, &mut state, rng) {
            return Err(format!("State machine check is failed: {:?}", e));
        }
        self.step += 1;
        if let StateType::Turn { player } = &self.state_type {
            self.turn += 1;
            if self.player >= *player {
                self.round += 1;
            }
            self.player = *player;
        }
        Ok(())
    }

    pub fn print(&self) {
        println!("Round: {}, turn: {}, step: {}", self.round, self.turn, self.step);
        println!("Done: {}", self.is_done());
        println!("Deck: {}", self.deck.len());
        for i in 0..self.deck.len() {
            println!("    {}) {:?}", i, self.deck[i]);
        }
        let winner = self.get_winner();
        println!("Players: {}", self.player_cards.len());
        for player in 0..self.player_cards.len() {
            if winner == Some(player) {
                print!("W");
            } else {
                print!(" ");
            }
            if player == self.player {
                print!("-> ");
            } else {
                print!("   ");
            }
            print!(" {})", player);
            if self.player_hands[player] > 0 {
                print!(" + ");
            } else {
                print!(" - ");
            }
            println!("{:?}", self.player_cards[player]);
        }
        println!("State: {:?}", self.state_type);
    }
}

pub fn get_example_settings() -> Settings {
    Settings { players_number: 6, cards_per_type: 3 }
}

pub fn get_example_actions() -> Vec<Action> {
    vec![
        Action {
            player: 0,
            action_type: ActionType::Income,
        },
        Action {
            player: 1,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 0,
            action_type: ActionType::BlockForeignAid,
        },
        Action {
            player: 0,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 2,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 1,
            action_type: ActionType::BlockForeignAid,
        },
        Action {
            player: 2,
            action_type: ActionType::Challenge,
        },
        Action {
            player: 1,
            action_type: ActionType::ShowCard(Card::Duke),
        },
        Action {
            player: 2,
            action_type: ActionType::RevealCard(Card::Ambassador),
        },
        Action {
            player: 1,
            action_type: ActionType::ShuffleDeck,
        },
        Action {
            player: 1,
            action_type: ActionType::TakeCard,
        },
        Action {
            player: 3,
            action_type: ActionType::Tax,
        },
        Action {
            player: 3,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 4,
            action_type: ActionType::Tax,
        },
        Action {
            player: 1,
            action_type: ActionType::Challenge,
        },
        Action {
            player: 4,
            action_type: ActionType::RevealCard(Card::Contessa),
        },
        Action {
            player: 5,
            action_type: ActionType::Steal(3),
        },
        Action {
            player: 5,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 3,
            action_type: ActionType::BlockSteal(Card::Ambassador),
        },
        Action {
            player: 3,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 0,
            action_type: ActionType::Tax,
        },
        Action {
            player: 0,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 1,
            action_type: ActionType::Steal(3),
        },
        Action {
            player: 1,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 3,
            action_type: ActionType::BlockSteal(Card::Captain),
        },
        Action {
            player: 1,
            action_type: ActionType::Challenge,
        },
        Action {
            player: 3,
            action_type: ActionType::ShowCard(Card::Captain),
        },
        Action {
            player: 1,
            action_type: ActionType::RevealCard(Card::Captain),
        },
        Action {
            player: 3,
            action_type: ActionType::ShuffleDeck,
        },
        Action {
            player: 3,
            action_type: ActionType::TakeCard,
        },
        Action {
            player: 2,
            action_type: ActionType::Tax,
        },
        Action {
            player: 2,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 3,
            action_type: ActionType::Steal(2),
        },
        Action {
            player: 3,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 3,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 4,
            action_type: ActionType::Income,
        },
        Action {
            player: 5,
            action_type: ActionType::Income,
        },
        Action {
            player: 0,
            action_type: ActionType::Assassinate(3),
        },
        Action {
            player: 0,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 0,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 3,
            action_type: ActionType::RevealCard(Card::Ambassador),
        },
        Action {
            player: 1,
            action_type: ActionType::Income,
        },
        Action {
            player: 2,
            action_type: ActionType::Tax,
        },
        Action {
            player: 2,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 3,
            action_type: ActionType::Steal(2),
        },
        Action {
            player: 3,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 3,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 4,
            action_type: ActionType::Income,
        },
        Action {
            player: 5,
            action_type: ActionType::Income,
        },
        Action {
            player: 0,
            action_type: ActionType::Tax,
        },
        Action {
            player: 0,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 1,
            action_type: ActionType::Income,
        },
        Action {
            player: 2,
            action_type: ActionType::Tax,
        },
        Action {
            player: 2,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 3,
            action_type: ActionType::Coup(0),
        },
        Action {
            player: 0,
            action_type: ActionType::RevealCard(Card::Assassin),
        },
        Action {
            player: 4,
            action_type: ActionType::Income,
        },
        Action {
            player: 5,
            action_type: ActionType::Income,
        },
        Action {
            player: 0,
            action_type: ActionType::Assassinate(2),
        },
        Action {
            player: 0,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 2,
            action_type: ActionType::BlockAssassination,
        },
        Action {
            player: 0,
            action_type: ActionType::Challenge,
        },
        Action {
            player: 2,
            action_type: ActionType::RevealCard(Card::Duke),
        },
        Action {
            player: 0,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 1,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 1,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 3,
            action_type: ActionType::Steal(4),
        },
        Action {
            player: 3,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 3,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 4,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 4,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 5,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 5,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 0,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 0,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 1,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 1,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 3,
            action_type: ActionType::Steal(5),
        },
        Action {
            player: 3,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 3,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 4,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 4,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 5,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 5,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 0,
            action_type: ActionType::Assassinate(1),
        },
        Action {
            player: 0,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 1,
            action_type: ActionType::BlockAssassination,
        },
        Action {
            player: 0,
            action_type: ActionType::Challenge,
        },
        Action {
            player: 1,
            action_type: ActionType::ShowCard(Card::Contessa),
        },
        Action {
            player: 0,
            action_type: ActionType::RevealCard(Card::Assassin),
        },
        Action {
            player: 1,
            action_type: ActionType::ShuffleDeck,
        },
        Action {
            player: 1,
            action_type: ActionType::TakeCard,
        },
        Action {
            player: 1,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 1,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 3,
            action_type: ActionType::Steal(1),
        },
        Action {
            player: 3,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 3,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 4,
            action_type: ActionType::Exchange,
        },
        Action {
            player: 4,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 4,
            action_type: ActionType::TakeCard,
        },
        Action {
            player: 4,
            action_type: ActionType::TakeCard,
        },
        Action {
            player: 4,
            action_type: ActionType::DropCard(Card::Ambassador),
        },
        Action {
            player: 4,
            action_type: ActionType::DropCard(Card::Contessa),
        },
        Action {
            player: 5,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 4,
            action_type: ActionType::BlockForeignAid,
        },
        Action {
            player: 5,
            action_type: ActionType::Challenge,
        },
        Action {
            player: 4,
            action_type: ActionType::ShowCard(Card::Duke),
        },
        Action {
            player: 5,
            action_type: ActionType::RevealCard(Card::Contessa),
        },
        Action {
            player: 4,
            action_type: ActionType::ShuffleDeck,
        },
        Action {
            player: 4,
            action_type: ActionType::TakeCard,
        },
        Action {
            player: 1,
            action_type: ActionType::Tax,
        },
        Action {
            player: 1,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 3,
            action_type: ActionType::Steal(1),
        },
        Action {
            player: 3,
            action_type: ActionType::PassChallenge,
        },
        Action {
            player: 3,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 4,
            action_type: ActionType::Assassinate(1),
        },
        Action {
            player: 1,
            action_type: ActionType::Challenge,
        },
        Action {
            player: 4,
            action_type: ActionType::ShowCard(Card::Assassin),
        },
        Action {
            player: 1,
            action_type: ActionType::RevealCard(Card::Duke),
        },
        Action {
            player: 4,
            action_type: ActionType::ShuffleDeck,
        },
        Action {
            player: 4,
            action_type: ActionType::TakeCard,
        },
        Action {
            player: 4,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 5,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 5,
            action_type: ActionType::PassBlock,
        },
        Action {
            player: 3,
            action_type: ActionType::Coup(5),
        },
        Action {
            player: 5,
            action_type: ActionType::RevealCard(Card::Captain),
        },
        Action {
            player: 4,
            action_type: ActionType::Tax,
        },
        Action {
            player: 3,
            action_type: ActionType::Challenge,
        },
        Action {
            player: 4,
            action_type: ActionType::ShowCard(Card::Duke),
        },
        Action {
            player: 3,
            action_type: ActionType::RevealCard(Card::Captain),
        },
        Action {
            player: 4,
            action_type: ActionType::ShuffleDeck,
        },
        Action {
            player: 4,
            action_type: ActionType::TakeCard,
        },
    ]
}

#[cfg(test)]
mod tests {
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    use super::*;

    #[test]
    fn income_should_add_coin_and_start_new_turn() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Income,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.player_coins[0], 3);
    }

    #[test]
    fn unblocked_foreign_aid_should_add_coins() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::ForeignAid,
            },
            Action {
                player: 0,
                action_type: ActionType::PassBlock,
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 1 });
        assert_eq!(game.player_coins[0], 4);
    }

    #[test]
    fn blocked_foreign_aid_should_not_add_coins() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::ForeignAid,
            },
            Action {
                player: 1,
                action_type: ActionType::BlockForeignAid,
            },
            Action {
                player: 1,
                action_type: ActionType::PassChallenge,
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 1 });
        assert_eq!(game.player_coins, vec![2, 2]);
    }

    #[test]
    fn failed_challenge_on_block_foreign_aid_should_fail_foreign_aid() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::ForeignAid,
            },
            Action {
                player: 1,
                action_type: ActionType::BlockForeignAid,
            },
            Action {
                player: 0,
                action_type: ActionType::Challenge,
            },
            Action {
                player: 1,
                action_type: ActionType::ShowCard(Card::Duke),
            },
            Action {
                player: 0,
                action_type: ActionType::RevealCard(game.player_cards[0][0]),
            },
            Action {
                player: 1,
                action_type: ActionType::ShuffleDeck,
            },
            Action {
                player: 1,
                action_type: ActionType::TakeCard,
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 1 });
        assert_eq!(game.player_coins, vec![2, 2]);
        assert_eq!(game.player_cards, vec![
            vec![Card::Contessa],
            vec![Card::Assassin, Card::Captain],
        ]);
        assert_eq!(game.revealed_cards, vec![Card::Ambassador]);
    }

    #[test]
    fn successful_challenge_for_block_foreign_aid_should_allow_first_aid() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::ForeignAid,
            },
            Action {
                player: 1,
                action_type: ActionType::BlockForeignAid,
            },
            Action {
                player: 0,
                action_type: ActionType::Challenge,
            },
            Action {
                player: 1,
                action_type: ActionType::RevealCard(game.player_cards[1][0]),
            },
            Action {
                player: 0,
                action_type: ActionType::PassBlock,
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 1 });
        assert_eq!(game.player_coins, vec![4, 2]);
        assert_eq!(game.player_cards, vec![
            vec![Card::Ambassador, Card::Contessa],
            vec![Card::Duke],
        ]);
        assert_eq!(game.revealed_cards, vec![Card::Captain]);
    }

    #[test]
    fn unchallenged_tax_should_add_coins() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Tax,
            },
            Action {
                player: 0,
                action_type: ActionType::PassChallenge,
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 1 });
        assert_eq!(game.player_coins[0], 5);
    }

    #[test]
    fn coup_should_subtract_coins_add_lead_to_lost_influence() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.player_coins[0] = 7;
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Coup(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.state_type, StateType::LostInfluence { player: 1, current_player: 0 });
        assert_eq!(game.player_coins[0], 0);
    }

    #[test]
    fn coup_against_not_active_player_should_return_error() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.player_coins[0] = 7;
        game.player_hands[1] = 0;
        game.player_cards[1].clear();
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Coup(1),
            }, &mut rng),
            Err(String::from("State machine check is failed: InvalidTarget"))
        );
        assert_eq!(game.state_type, StateType::Turn { player: 0 });
    }

    #[test]
    fn block_steal_after_steal_should_add_counteraction() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Steal(1),
            },
            Action {
                player: 0,
                action_type: ActionType::PassChallenge,
            },
            Action {
                player: 1,
                action_type: ActionType::BlockSteal(Card::Ambassador),
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::BlockSteal { player: 1, target: 0, card: Card::Ambassador });
    }

    #[test]
    fn block_steal_should_fail_for_non_targeted_player() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 3, cards_per_type: 2 }, &mut rng);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Steal(1),
            },
            Action {
                player: 0,
                action_type: ActionType::PassChallenge,
            },
            Action {
                player: 2,
                action_type: ActionType::BlockSteal(Card::Captain),
            },
        ];
        assert_eq!(
            play_actions(&actions, &mut game, &mut rng),
            Err(String::from("State machine check is failed: InvalidTarget"))
        );
        assert_eq!(game.state_type, StateType::Steal { player: 0, target: 1, can_challenge: false });
    }

    #[test]
    fn successful_challenged_block_steal_should_prevent_steal() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.player_cards[1][0] = Card::Ambassador;
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Steal(1),
            },
            Action {
                player: 0,
                action_type: ActionType::PassChallenge,
            },
            Action {
                player: 1,
                action_type: ActionType::BlockSteal(Card::Ambassador),
            },
            Action {
                player: 0,
                action_type: ActionType::Challenge,
            },
            Action {
                player: 1,
                action_type: ActionType::ShowCard(Card::Ambassador),
            },
            Action {
                player: 0,
                action_type: ActionType::RevealCard(game.player_cards[0][0]),
            },
            Action {
                player: 1,
                action_type: ActionType::ShuffleDeck,
            },
            Action {
                player: 1,
                action_type: ActionType::TakeCard,
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 1 });
        assert_eq!(game.player_coins[0], 2);
        assert_eq!(game.player_coins[1], 2);
    }

    #[test]
    fn successful_steal_should_transfer_coins_from_target_to_theft() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Steal(1),
            },
            Action {
                player: 0,
                action_type: ActionType::PassChallenge,
            },
            Action {
                player: 0,
                action_type: ActionType::PassBlock,
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 1 });
        assert_eq!(game.player_coins[0], 4);
        assert_eq!(game.player_coins[1], 0);
    }

    #[test]
    fn successful_steal_challenge_should_prevent_stealing() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Steal(1),
            },
            Action {
                player: 1,
                action_type: ActionType::Challenge,
            },
            Action {
                player: 0,
                action_type: ActionType::RevealCard(game.player_cards[0][0]),
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 1 });
        assert_eq!(game.player_coins[0], 2);
        assert_eq!(game.player_coins[1], 2);
    }

    #[test]
    fn failed_steal_challenge_should_transfer_coins() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.player_cards[0][0] = Card::Captain;
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Steal(1),
            },
            Action {
                player: 1,
                action_type: ActionType::Challenge,
            },
            Action {
                player: 0,
                action_type: ActionType::ShowCard(game.player_cards[0][0]),
            },
            Action {
                player: 1,
                action_type: ActionType::RevealCard(game.player_cards[1][0]),
            },
            Action {
                player: 0,
                action_type: ActionType::ShuffleDeck,
            },
            Action {
                player: 0,
                action_type: ActionType::TakeCard,
            },
            Action {
                player: 0,
                action_type: ActionType::PassBlock,
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 1 });
        assert_eq!(game.player_coins[0], 4);
        assert_eq!(game.player_coins[1], 0);
    }

    #[test]
    fn failed_steal_challenge_for_targeted_player_with_one_card_should_transfer_coins() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.player_cards[0][0] = Card::Captain;
        game.player_hands[1] = 1;
        game.player_cards_counter[1] = 1;
        game.player_cards[1].remove(1);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Steal(1),
            },
            Action {
                player: 1,
                action_type: ActionType::Challenge,
            },
            Action {
                player: 0,
                action_type: ActionType::ShowCard(game.player_cards[0][0]),
            },
            Action {
                player: 1,
                action_type: ActionType::RevealCard(game.player_cards[1][0]),
            },
            Action {
                player: 0,
                action_type: ActionType::ShuffleDeck,
            },
            Action {
                player: 0,
                action_type: ActionType::TakeCard,
            },
            Action {
                player: 0,
                action_type: ActionType::PassBlock,
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 0 });
        assert_eq!(game.player_coins[0], 4);
        assert_eq!(game.player_coins[1], 0);
        assert_eq!(game.player_hands[1], 0);
        assert_eq!(game.player_cards_counter[1], 0);
        assert_eq!(game.player_cards[1], vec![]);
    }

    #[test]
    fn assassinate_should_subtract_coins() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.player_coins[0] = 3;
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Assassinate(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.player_coins[0], 0);
    }

    #[test]
    fn block_assassinate_should_fail_for_non_targeted_player() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 3, cards_per_type: 2 }, &mut rng);
        game.player_coins[0] = 3;
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Assassinate(1),
            },
            Action {
                player: 0,
                action_type: ActionType::PassChallenge,
            },
            Action {
                player: 2,
                action_type: ActionType::BlockAssassination,
            },
        ];
        assert_eq!(
            play_actions(&actions, &mut game, &mut rng),
            Err(String::from("State machine check is failed: InvalidTarget"))
        );
    }

    #[test]
    fn failed_assassination_challenge_should_end_game_for_targeted_player() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 3, cards_per_type: 2 }, &mut rng);
        game.player_coins[0] = 3;
        game.player_cards[0][0] = Card::Assassin;
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Assassinate(1),
            },
            Action {
                player: 1,
                action_type: ActionType::Challenge,
            },
            Action {
                player: 0,
                action_type: ActionType::ShowCard(Card::Assassin),
            },
            Action {
                player: 1,
                action_type: ActionType::RevealCard(game.player_cards[1][0]),
            },
            Action {
                player: 0,
                action_type: ActionType::ShuffleDeck,
            },
            Action {
                player: 0,
                action_type: ActionType::TakeCard,
            },
            Action {
                player: 0,
                action_type: ActionType::PassBlock,
            },
            Action {
                player: 1,
                action_type: ActionType::RevealCard(game.player_cards[1][1]),
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 2 });
        assert_eq!(game.player_coins[0], 0);
        assert_eq!(game.player_hands[1], 0);
        assert_eq!(game.player_cards_counter[1], 0);
        assert_eq!(game.player_cards[1], vec![]);
        assert_eq!(game.revealed_cards, vec![Card::Ambassador, Card::Duke]);
    }

    #[test]
    fn failed_assassination_challenge_for_targeted_player_with_one_card_should_not_allow_it_to_block() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 3, cards_per_type: 2 }, &mut rng);
        game.player_coins[0] = 3;
        game.player_cards[0][0] = Card::Assassin;
        game.player_hands[1] = 1;
        game.player_cards_counter[1] = 1;
        game.player_cards[1].remove(1);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Assassinate(1),
            },
            Action {
                player: 1,
                action_type: ActionType::Challenge,
            },
            Action {
                player: 0,
                action_type: ActionType::ShowCard(Card::Assassin),
            },
            Action {
                player: 1,
                action_type: ActionType::RevealCard(game.player_cards[1][0]),
            },
            Action {
                player: 0,
                action_type: ActionType::ShuffleDeck,
            },
            Action {
                player: 0,
                action_type: ActionType::TakeCard,
            },
            Action {
                player: 0,
                action_type: ActionType::PassBlock,
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 2 });
        assert_eq!(game.player_coins[0], 0);
        assert_eq!(game.player_hands[1], 0);
        assert_eq!(game.player_cards_counter[1], 0);
        assert_eq!(game.player_cards[1], vec![]);
        assert_eq!(game.revealed_cards, vec![Card::Ambassador]);
    }

    #[test]
    fn successful_exchange_should_replace_cards_with_deck() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 3, cards_per_type: 2 }, &mut rng);
        game.player_coins[0] = 3;
        game.player_cards[0][0] = Card::Assassin;
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Exchange,
            },
            Action {
                player: 0,
                action_type: ActionType::PassChallenge,
            },
            Action {
                player: 0,
                action_type: ActionType::TakeCard,
            },
            Action {
                player: 0,
                action_type: ActionType::TakeCard,
            },
            Action {
                player: 0,
                action_type: ActionType::DropCard(game.player_cards[0][0]),
            },
            Action {
                player: 0,
                action_type: ActionType::DropCard(game.player_cards[0][1]),
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 1 });
        assert_eq!(game.player_cards[0], vec![Card::Captain, Card::Duke]);
    }

    #[test]
    fn successful_challenge_for_exchange_should_prevent_exchange() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Exchange,
            },
            Action {
                player: 1,
                action_type: ActionType::Challenge,
            },
            Action {
                player: 0,
                action_type: ActionType::RevealCard(game.player_cards[0][0]),
            },
        ];
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert_eq!(game.state_type, StateType::Turn { player: 1 });
    }

    #[test]
    fn play_full_game_should_set_a_winner() {
        let actions = get_example_actions();
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(get_example_settings(), &mut rng);
        assert_eq!(play_actions(&actions, &mut game, &mut rng), Ok(()));
        assert!(game.is_done());
        assert_eq!(game.get_winner(), Some(4));
        assert_eq!(game.step(), actions.len());
        assert_eq!(game.turn(), 45);
        assert_eq!(game.round(), 9);
    }

    fn play_actions<R: Rng>(actions: &[Action], game: &mut Game, rng: &mut R) -> Result<(), String> {
        for i in 0..actions.len() {
            let action = &actions[i];
            let view = game.get_player_view(action.player);
            let available_actions = get_available_actions(&view.state_type, &view.player_coins, &view.player_hands);
            game.print();
            println!("Play {:?}", action);
            match game.play(action, rng) {
                Ok(_) => {
                    assert!(available_actions.contains(action), "{}) played action {:?} is not considered as available: {:?}", i, action, available_actions);
                }
                Err(e) => {
                    assert!(!available_actions.contains(action), "{}) failed action {:?} is considered as available: {:?}", i, action, available_actions);
                    return Err(e);
                }
            }
        }
        game.print();
        Ok(())
    }
}
