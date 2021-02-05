use std::rc::Rc;

use itertools::Itertools;
use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

pub const CARDS_PER_PLAYER: usize = 2;
pub const MAX_CARDS_TO_EXCHANGE: usize = 2;
pub const ASSASSINATION_COST: usize = 3;
pub const INCOME: usize = 1;
pub const FOREIGN_AID: usize = 2;
pub const TAX: usize = 3;
pub const MAX_STEAL: usize = 2;
pub const COUP_COST: usize = 7;
pub const MAX_COINS: usize = 10;

pub struct ConstRng;

impl rand::RngCore for ConstRng {
    fn next_u32(&mut self) -> u32 {
        42
    }

    fn next_u64(&mut self) -> u64 {
        42
    }

    fn fill_bytes(&mut self, _: &mut [u8]) {}

    fn try_fill_bytes(&mut self, _: &mut [u8]) -> Result<(), rand::Error> {
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Card {
    Unknown,
    Assassin,
    Ambassador,
    Captain,
    Contessa,
    Duke,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub player: usize,
    pub action_type: ActionType,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum ActionType {
    Income,
    ForeignAid,
    Coup(usize),
    Tax,
    Assassinate(usize),
    Exchange,
    Steal(usize),
    BlockForeignAid,
    BlockAssassination,
    BlockSteal(Card),
    PassChallenge,
    PassBlock,
    Challenge,
    ShowCard(Card),
    RevealCard(Card),
    TakeCard,
    ShuffleDeck,
    DropCard(Card),
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    InvalidPlayer,
    InvalidTarget,
    InvalidAction,
    InvalidCard,
    InvalidSource,
    NotEnoughCoins,
    TooManyCoins,
    InactivePlayer,
}

#[derive(Debug, Clone, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub enum StateType {
    Turn {
        player: usize,
    },
    ForeignAid {
        player: usize,
    },
    Tax {
        player: usize,
    },
    Exchange {
        player: usize,
    },
    Assassination {
        player: usize,
        target: usize,
        can_challenge: bool,
    },
    Steal {
        player: usize,
        target: usize,
        can_challenge: bool,
    },
    Challenge {
        current_player: usize,
        source: Rc<StateType>,
        state: ChallengeState,
    },
    BlockForeignAid {
        player: usize,
        target: usize,
    },
    NeedCards {
        player: usize,
        count: usize,
    },
    TookCards {
        player: usize,
        count: usize,
    },
    DroppedCard {
        player: usize,
        left: usize,
    },
    BlockAssassination {
        player: usize,
        target: usize,
    },
    BlockSteal {
        player: usize,
        target: usize,
        card: Card,
    },
    LostInfluence {
        player: usize,
        current_player: usize,
    },
}

pub trait PlayerCards {
    fn has_card(&self, card: Card) -> bool;
    fn count(&self) -> usize;
    fn add_card(&mut self, card: Card);
    fn drop_card(&mut self, card: Card);
}

impl PlayerCards for Vec<Card> {
    fn has_card(&self, card: Card) -> bool {
        self.contains(&card)
    }

    fn count(&self) -> usize {
        self.len()
    }

    fn add_card(&mut self, card: Card) {
        self.push(card);
        self.sort();
    }

    fn drop_card(&mut self, card: Card) {
        let index = self.iter()
            .find_position(|v| **v == card)
            .map(|(index, _)| index)
            .unwrap();
        self.remove(index);
    }
}

pub trait Deck {
    fn count(&self) -> usize;
    fn pop_card(&mut self) -> Card;
    fn push_card(&mut self, card: Card);
    fn shuffle<R: Rng>(&mut self, rng: &mut R);
}

impl Deck for Vec<Card> {
    fn count(&self) -> usize {
        self.len()
    }

    fn pop_card(&mut self) -> Card {
        self.pop().unwrap()
    }

    fn push_card(&mut self, card: Card) {
        self.push(card)
    }

    fn shuffle<R: Rng>(&mut self, rng: &mut R) {
        SliceRandom::shuffle(&mut self[..], rng);
    }
}

#[derive(Debug)]
pub struct State<'a, P: PlayerCards + Sized, D: Deck> {
    pub state_type: &'a mut StateType,
    pub player_coins: &'a mut [usize],
    pub player_hands: &'a mut [usize],
    pub player_cards_counter: &'a mut [usize],
    pub player_cards: &'a mut [P],
    pub deck: &'a mut D,
    pub revealed_cards: &'a mut Vec<Card>,
}

pub fn play_action<'a, P, D, R>(action: &Action, state: &mut State<'a, P, D>, rng: &mut R) -> Result<(), Error>
    where P: PlayerCards,
          D: Deck,
          R: Rng {
    if state.player_hands[action.player] == 0 {
        return Err(Error::InactivePlayer);
    }
    let new_state_type = match &state.state_type {
        StateType::Turn { player } => {
            on_turn(*player, state.player_coins, state.player_hands, action)
        }
        StateType::ForeignAid { player } => {
            on_foreign_aid(*player, state.player_coins, state.player_hands, action)
        }
        StateType::Tax { player } => {
            on_tax(*player, state.player_coins, state.player_hands, action)
        }
        StateType::Exchange { player } => {
            on_exchange(*player, state.player_hands, state.deck, action)
        }
        StateType::Assassination { player, target, can_challenge } => {
            on_assassination(*player, *target, *can_challenge, state.player_hands, action)
        }
        StateType::Steal { player, target, can_challenge } => {
            on_steal(*player, *target, *can_challenge, state.player_coins, state.player_hands, action)
        }
        StateType::Challenge { current_player, source, state: challenge_state } => {
            on_challenge(*current_player, source, challenge_state, state.player_coins, state.player_hands, state.player_cards_counter, state.player_cards, state.deck, state.revealed_cards, action, rng)
        }
        StateType::BlockForeignAid { player, target } => {
            on_block_foreign_aid(*player, *target, state.player_hands, action)
        }
        StateType::NeedCards { player, count } => {
            on_need_cards(*player, *count, state.player_hands, state.player_cards_counter, state.player_cards, state.deck, action)
        }
        StateType::TookCards { player, count } => {
            on_took_cards(*player, *count, state.player_hands, state.player_cards_counter, state.player_cards, state.deck, action)
        }
        StateType::DroppedCard { player, left } => {
            on_dropped_cards(*player, *left, state.player_hands, state.player_cards_counter, state.player_cards, state.deck, action)
        }
        StateType::BlockAssassination { player, target } => {
            on_block_assassination(*player, *target, state.player_hands, action)
        }
        StateType::BlockSteal { player, target, card } => {
            on_block_steal(*player, *target, *card, state.player_hands, action)
        }
        StateType::LostInfluence { player, current_player } => {
            on_lost_influence(*player, *current_player, state.player_hands, state.player_cards_counter, state.player_cards, state.revealed_cards, action)
        }
    }?;
    *state.state_type = new_state_type;
    Ok(())
}

fn on_turn(player: usize, player_coins: &mut [usize], player_hands: &[usize],
           action: &Action) -> Result<StateType, Error> {
    if player != action.player {
        return Err(Error::InvalidPlayer);
    }
    if player_coins[player] >= MAX_COINS && !matches!(action.action_type, ActionType::Coup(..)) {
        return Err(Error::TooManyCoins);
    }
    match &action.action_type {
        ActionType::Income => {
            player_coins[player] += INCOME;
            Ok(StateType::Turn { player: get_next_player(player, player_hands) })
        }
        ActionType::ForeignAid => Ok(StateType::ForeignAid { player }),
        ActionType::Tax => Ok(StateType::Tax { player }),
        ActionType::Exchange => Ok(StateType::Exchange { player }),
        ActionType::Coup(target) => {
            if *target == player || player_hands[*target] == 0 {
                return Err(Error::InvalidTarget);
            }
            if player_coins[player] < COUP_COST {
                return Err(Error::NotEnoughCoins);
            }
            player_coins[player] -= COUP_COST;
            Ok(StateType::LostInfluence {
                player: *target,
                current_player: player,
            })
        }
        ActionType::Assassinate(target) => {
            if *target == player || player_hands[*target] == 0 {
                return Err(Error::InvalidTarget);
            }
            if player_coins[player] < ASSASSINATION_COST {
                return Err(Error::NotEnoughCoins);
            }
            player_coins[player] -= ASSASSINATION_COST;
            Ok(StateType::Assassination { player, target: *target, can_challenge: true })
        }
        ActionType::Steal(target) => {
            if *target == player || player_hands[*target] == 0 {
                return Err(Error::InvalidTarget);
            }
            Ok(StateType::Steal { player, target: *target, can_challenge: true })
        }
        _ => return Err(Error::InvalidAction),
    }
}

fn on_foreign_aid(player: usize, player_coins: &mut [usize], player_hands: &[usize],
                  action: &Action) -> Result<StateType, Error> {
    match &action.action_type {
        ActionType::PassBlock => {
            if player != action.player {
                return Err(Error::InvalidPlayer);
            }
            player_coins[player] += FOREIGN_AID;
            Ok(StateType::Turn { player: get_next_player(player, player_hands) })
        }
        ActionType::BlockForeignAid => {
            if player == action.player {
                return Err(Error::InvalidTarget);
            }
            Ok(StateType::BlockForeignAid { player: action.player, target: player })
        }
        _ => return Err(Error::InvalidAction),
    }
}

fn on_tax(player: usize, player_coins: &mut [usize], player_hands: &[usize],
          action: &Action) -> Result<StateType, Error> {
    match &action.action_type {
        ActionType::PassChallenge => {
            if player != action.player {
                return Err(Error::InvalidPlayer);
            }
            player_coins[player] += TAX;
            Ok(StateType::Turn { player: get_next_player(player, player_hands) })
        }
        ActionType::Challenge => {
            if player == action.player {
                return Err(Error::InvalidTarget);
            }
            Ok(StateType::Challenge {
                current_player: player,
                state: ChallengeState::Initial { initiator: action.player, target: player, card: Card::Duke },
                source: Rc::new(StateType::Tax { player }),
            })
        }
        _ => return Err(Error::InvalidAction),
    }
}

fn on_exchange<D: Deck>(player: usize, player_hands: &[usize], deck: &D,
                        action: &Action) -> Result<StateType, Error> {
    match &action.action_type {
        ActionType::PassChallenge => {
            if player != action.player {
                return Err(Error::InvalidPlayer);
            }
            start_exchange(player, player_hands, deck)
        }
        ActionType::Challenge => {
            if player == action.player {
                return Err(Error::InvalidTarget);
            }
            Ok(StateType::Challenge {
                current_player: player,
                state: ChallengeState::Initial { initiator: action.player, target: player, card: Card::Ambassador },
                source: Rc::new(StateType::Exchange { player }),
            })
        }
        _ => Err(Error::InvalidAction),
    }
}

fn on_assassination(player: usize, target: usize, can_challenge: bool, player_hands: &[usize],
                    action: &Action) -> Result<StateType, Error> {
    if can_challenge {
        match &action.action_type {
            ActionType::PassChallenge => {
                if player != action.player {
                    return Err(Error::InvalidPlayer);
                }
                Ok(StateType::Assassination { player, target, can_challenge: false })
            }
            ActionType::Challenge => {
                if player == action.player {
                    return Err(Error::InvalidTarget);
                }
                Ok(StateType::Challenge {
                    current_player: player,
                    state: ChallengeState::Initial { initiator: action.player, target: player, card: Card::Assassin },
                    source: Rc::new(StateType::Assassination { player, target, can_challenge: true }),
                })
            }
            _ => return Err(Error::InvalidAction),
        }
    } else {
        match &action.action_type {
            ActionType::PassBlock => {
                if player != action.player {
                    return Err(Error::InvalidPlayer);
                }
                if player_hands[target] == 0 {
                    Ok(StateType::Turn { player: get_next_player(player, player_hands) })
                } else {
                    Ok(StateType::LostInfluence { player: target, current_player: player })
                }
            }
            ActionType::BlockAssassination => {
                if player == action.player || target != action.player {
                    return Err(Error::InvalidTarget);
                }
                Ok(StateType::BlockAssassination { player: action.player, target: player })
            }
            _ => Err(Error::InvalidAction),
        }
    }
}

fn on_steal(player: usize, target: usize, can_challenge: bool, player_coins: &mut [usize],
            player_hands: &[usize], action: &Action) -> Result<StateType, Error> {
    if can_challenge {
        match &action.action_type {
            ActionType::PassChallenge => {
                if player != action.player {
                    return Err(Error::InvalidPlayer);
                }
                Ok(StateType::Steal { player, target, can_challenge: false })
            }
            ActionType::Challenge => {
                if player == action.player {
                    return Err(Error::InvalidTarget);
                }
                Ok(StateType::Challenge {
                    current_player: player,
                    state: ChallengeState::Initial { initiator: action.player, target: player, card: Card::Captain },
                    source: Rc::new(StateType::Steal { player, target, can_challenge: true }),
                })
            }
            _ => Err(Error::InvalidAction),
        }
    } else {
        match &action.action_type {
            ActionType::PassBlock => {
                if player != action.player {
                    return Err(Error::InvalidPlayer);
                }
                let coins = player_coins[target].min(MAX_STEAL);
                player_coins[target] -= coins;
                player_coins[player] += coins;
                Ok(StateType::Turn { player: get_next_player(player, player_hands) })
            }
            ActionType::BlockSteal(card) => {
                if player == action.player || target != action.player {
                    return Err(Error::InvalidTarget);
                }
                if !matches!(card, Card::Ambassador | Card::Captain) {
                    return Err(Error::InvalidCard);
                }
                Ok(StateType::BlockSteal { player: action.player, target: player, card: *card })
            }
            _ => Err(Error::InvalidAction),
        }
    }
}

fn on_challenge<P, D, R>(current_player: usize, source: &Rc<StateType>, state: &ChallengeState, player_coins: &mut [usize],
                         player_hands: &mut [usize], player_cards_counter: &mut [usize], player_cards: &mut [P], deck: &mut D,
                         revealed_cards: &mut Vec<Card>, action: &Action, rng: &mut R) -> Result<StateType, Error>
    where P: PlayerCards,
          D: Deck,
          R: Rng {
    match play_challenge_action(state, player_hands, player_cards_counter, player_cards, deck, revealed_cards, action, rng)? {
        ChallengeState::TookCard => match &**source {
            StateType::Tax { player } => {
                player_coins[*player] += TAX;
                Ok(StateType::Turn { player: get_next_player(current_player, player_hands) })
            }
            StateType::BlockForeignAid { .. } | StateType::BlockAssassination { .. }
            | StateType::BlockSteal { .. } => {
                Ok(StateType::Turn { player: get_next_player(current_player, player_hands) })
            }
            StateType::Exchange { player } => {
                start_exchange(*player, player_hands, deck)
            }
            StateType::Assassination { player, target, .. } => {
                Ok(StateType::Assassination { player: *player, target: *target, can_challenge: false })
            }
            StateType::Steal { player, target, .. } => {
                Ok(StateType::Steal { player: *player, target: *target, can_challenge: false })
            }
            _ => Err(Error::InvalidSource),
        },
        ChallengeState::TargetRevealedCard => match &**source {
            StateType::BlockForeignAid { target, .. } => {
                Ok(StateType::ForeignAid { player: *target })
            }
            StateType::BlockAssassination { player, target, .. } => {
                Ok(StateType::Assassination { player: *target, target: *player, can_challenge: false })
            }
            StateType::BlockSteal { player, target, .. } => {
                Ok(StateType::Steal { player: *target, target: *player, can_challenge: false })
            }
            StateType::Tax { .. } | StateType::Exchange { .. }
            | StateType::Assassination { .. } | StateType::Steal { .. } => {
                Ok(StateType::Turn { player: get_next_player(current_player, player_hands) })
            }
            _ => Err(Error::InvalidSource),
        }
        v => {
            Ok(StateType::Challenge { current_player, state: v, source: source.clone() })
        }
    }
}

fn on_block_foreign_aid(player: usize, target: usize, player_hands: &[usize],
                        action: &Action) -> Result<StateType, Error> {
    match &action.action_type {
        ActionType::PassChallenge => {
            if player != action.player {
                return Err(Error::InvalidPlayer);
            }
            Ok(StateType::Turn { player: get_next_player(target, player_hands) })
        }
        ActionType::Challenge => {
            if player == action.player {
                return Err(Error::InvalidTarget);
            }
            Ok(StateType::Challenge {
                current_player: target,
                state: ChallengeState::Initial { initiator: action.player, target: player, card: Card::Duke },
                source: Rc::new(StateType::BlockForeignAid { player, target }),
            })
        }
        _ => Err(Error::InvalidAction),
    }
}

fn on_need_cards<P, D>(player: usize, count: usize, player_hands: &[usize],
                       player_cards_counter: &mut [usize], player_cards: &mut [P], deck: &mut D,
                       action: &Action) -> Result<StateType, Error>
    where P: PlayerCards,
          D: Deck {
    if player != action.player {
        return Err(Error::InvalidPlayer);
    }
    match &action.action_type {
        ActionType::TakeCard => {
            player_cards[player].add_card(deck.pop_card());
            player_cards_counter[player] += 1;
            if count == 1 {
                Ok(StateType::TookCards { player, count: player_cards_counter[player] - player_hands[player] })
            } else {
                Ok(StateType::NeedCards { player, count: count - 1 })
            }
        }
        _ => Err(Error::InvalidAction),
    }
}

fn on_took_cards<P, D>(player: usize, count: usize, player_hands: &[usize], player_cards_counter: &mut [usize],
                       player_cards: &mut [P], deck: &mut D, action: &Action) -> Result<StateType, Error>
    where P: PlayerCards,
          D: Deck {
    match &action.action_type {
        ActionType::DropCard(card) => {
            if player != action.player {
                return Err(Error::InvalidPlayer);
            }
            if !player_cards[player].has_card(*card) {
                return Err(Error::InvalidCard);
            }
            player_cards[player].drop_card(*card);
            player_cards_counter[player] -= 1;
            deck.push_card(*card);
            if count == 1 {
                Ok(StateType::Turn { player: get_next_player(player, player_hands) })
            } else {
                Ok(StateType::TookCards { player, count: count - 1 })
            }
        }
        _ => Err(Error::InvalidAction),
    }
}

fn on_dropped_cards<P, D>(player: usize, left: usize, player_hands: &[usize], player_cards_counter: &mut [usize],
                          player_cards: &mut [P], deck: &mut D, action: &Action) -> Result<StateType, Error>
    where P: PlayerCards,
          D: Deck {
    match &action.action_type {
        ActionType::DropCard(card) => {
            if player != action.player {
                return Err(Error::InvalidPlayer);
            }
            if !player_cards[player].has_card(*card) {
                return Err(Error::InvalidCard);
            }
            player_cards[player].drop_card(*card);
            player_cards_counter[player] -= 1;
            deck.push_card(*card);
            if left == 1 {
                Ok(StateType::Turn { player: get_next_player(player, player_hands) })
            } else {
                Ok(StateType::DroppedCard { player, left: left - 1 })
            }
        }
        _ => Err(Error::InvalidAction),
    }
}

fn on_block_assassination(player: usize, target: usize, player_hands: &[usize],
                          action: &Action) -> Result<StateType, Error> {
    match &action.action_type {
        ActionType::PassChallenge => {
            if player != action.player {
                return Err(Error::InvalidPlayer);
            }
            Ok(StateType::Turn { player: get_next_player(target, player_hands) })
        }
        ActionType::Challenge => {
            if player == action.player {
                return Err(Error::InvalidPlayer);
            }
            Ok(StateType::Challenge {
                current_player: target,
                state: ChallengeState::Initial { initiator: action.player, target: player, card: Card::Contessa },
                source: Rc::new(StateType::BlockAssassination { player, target }),
            })
        }
        _ => Err(Error::InvalidAction),
    }
}

fn on_block_steal(player: usize, target: usize, card: Card, player_hands: &[usize],
                  action: &Action) -> Result<StateType, Error> {
    match &action.action_type {
        ActionType::PassChallenge => {
            if player != action.player {
                return Err(Error::InvalidPlayer);
            }
            Ok(StateType::Turn { player: get_next_player(target, player_hands) })
        }
        ActionType::Challenge => {
            if player == action.player {
                return Err(Error::InvalidPlayer);
            }
            Ok(StateType::Challenge {
                current_player: target,
                state: ChallengeState::Initial { initiator: action.player, target: player, card },
                source: Rc::new(StateType::BlockSteal { player, target, card }),
            })
        }
        _ => Err(Error::InvalidAction),
    }
}

fn on_lost_influence<P>(player: usize, current_turn_player: usize, player_hands: &mut [usize],
                        player_cards_counter: &mut [usize], player_cards: &mut [P],
                        revealed_cards: &mut Vec<Card>, action: &Action) -> Result<StateType, Error>
    where P: PlayerCards {
    match &action.action_type {
        ActionType::RevealCard(card) => {
            if player != action.player {
                return Err(Error::InvalidPlayer);
            }
            if !player_cards[player].has_card(*card) {
                return Err(Error::InvalidCard);
            }
            player_cards[player].drop_card(*card);
            player_hands[player] -= 1;
            player_cards_counter[player] -= 1;
            revealed_cards.push(*card);
            Ok(StateType::Turn { player: get_next_player(current_turn_player, player_hands) })
        }
        _ => Err(Error::InvalidAction),
    }
}

fn start_exchange<D: Deck>(player: usize, player_hands: &[usize], deck: &D) -> Result<StateType, Error> {
    match MAX_CARDS_TO_EXCHANGE.min(deck.count()) {
        0 => Ok(StateType::Turn { player: get_next_player(player, player_hands) }),
        count => Ok(StateType::NeedCards { player, count }),
    }
}

fn get_next_player(mut player: usize, player_hands: &[usize]) -> usize {
    while player_hands[(player + 1) % player_hands.len()] == 0 {
        player += 1
    }
    (player + 1) % player_hands.len()
}

#[derive(Debug, Clone, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
pub enum ChallengeState {
    Initial {
        initiator: usize,
        target: usize,
        card: Card,
    },
    ShownCard {
        initiator: usize,
        target: usize,
    },
    InitiatorRevealedCard {
        target: usize,
    },
    DeckShuffled {
        target: usize,
    },
    TookCard,
    TargetRevealedCard,
}

fn play_challenge_action<P, D, R>(state: &ChallengeState, player_hands: &mut [usize], player_cards_counter: &mut [usize],
                                  player_cards: &mut [P], deck: &mut D, revealed_cards: &mut Vec<Card>,
                                  action: &Action, rng: &mut R) -> Result<ChallengeState, Error>
    where P: PlayerCards,
          D: Deck,
          R: Rng {
    match state {
        ChallengeState::Initial { initiator, target, card } => {
            on_challenge_initial(*initiator, *target, *card, player_hands, player_cards_counter, player_cards, deck, revealed_cards, action)
        }
        ChallengeState::ShownCard { initiator, target } => {
            on_challenge_shown_card(*initiator, *target, player_hands, player_cards_counter, player_cards, revealed_cards, action)
        }
        ChallengeState::InitiatorRevealedCard { target } => {
            on_challenge_initiator_revealed_card(*target, deck, action, rng)
        }
        ChallengeState::DeckShuffled { target } => {
            on_challenge_deck_shuffled(*target, player_cards_counter, player_cards, deck, action)
        }
        _ => Err(Error::InvalidAction),
    }
}

fn on_challenge_initial<P, D>(initiator: usize, target: usize, card: Card, player_hands: &mut [usize],
                              player_cards_counter: &mut [usize], player_cards: &mut [P], deck: &mut D,
                              revealed_cards: &mut Vec<Card>, action: &Action) -> Result<ChallengeState, Error>
    where P: PlayerCards,
          D: Deck {
    if target != action.player {
        return Err(Error::InvalidPlayer);
    }
    match &action.action_type {
        ActionType::ShowCard(shown_card) => {
            if *shown_card != card || !player_cards[target].has_card(card) {
                return Err(Error::InvalidCard);
            }
            player_cards[target].drop_card(card);
            player_cards_counter[target] -= 1;
            deck.push_card(card);
            Ok(ChallengeState::ShownCard { initiator, target })
        }
        ActionType::RevealCard(revealed_card) => {
            if !player_cards[target].has_card(*revealed_card) {
                return Err(Error::InvalidCard);
            }
            player_cards[target].drop_card(*revealed_card);
            player_hands[target] -= 1;
            player_cards_counter[target] -= 1;
            revealed_cards.push(*revealed_card);
            Ok(ChallengeState::TargetRevealedCard)
        }
        _ => Err(Error::InvalidAction),
    }
}

fn on_challenge_shown_card<P>(initiator: usize, target: usize, player_hands: &mut [usize],
                              player_cards_counter: &mut [usize], player_cards: &mut [P],
                              revealed_cards: &mut Vec<Card>, action: &Action) -> Result<ChallengeState, Error>
    where P: PlayerCards {
    if initiator != action.player {
        return Err(Error::InvalidPlayer);
    }
    match &action.action_type {
        ActionType::RevealCard(card) => {
            if !player_cards[initiator].has_card(*card) {
                return Err(Error::InvalidCard);
            }
            player_cards[initiator].drop_card(*card);
            player_hands[initiator] -= 1;
            player_cards_counter[initiator] -= 1;
            revealed_cards.push(*card);
            Ok(ChallengeState::InitiatorRevealedCard { target })
        }
        _ => Err(Error::InvalidAction),
    }
}

fn on_challenge_initiator_revealed_card<D, R>(target: usize, deck: &mut D, action: &Action,
                                              rng: &mut R) -> Result<ChallengeState, Error>
    where D: Deck,
          R: Rng {
    if target != action.player {
        return Err(Error::InvalidPlayer);
    }
    match &action.action_type {
        ActionType::ShuffleDeck => {
            deck.shuffle(rng);
            Ok(ChallengeState::DeckShuffled { target })
        }
        _ => Err(Error::InvalidAction),
    }
}

fn on_challenge_deck_shuffled<P, D>(target: usize, player_cards_counter: &mut [usize], player_cards: &mut [P],
                                    deck: &mut D, action: &Action) -> Result<ChallengeState, Error>
    where P: PlayerCards,
          D: Deck {
    if target != action.player {
        return Err(Error::InvalidPlayer);
    }
    match &action.action_type {
        ActionType::TakeCard => {
            player_cards[target].add_card(deck.pop_card());
            player_cards_counter[target] += 1;
            Ok(ChallengeState::TookCard)
        }
        _ => Err(Error::InvalidAction),
    }
}

#[cfg(test)]
mod tests {
    use crate::fsm::*;

    #[derive(Debug)]
    struct TestState {
        state_type: StateType,
        player_coins: Vec<usize>,
        player_hands: Vec<usize>,
        player_cards_counter: Vec<usize>,
        player_cards: Vec<Vec<Card>>,
        deck: Vec<Card>,
        revealed_cards: Vec<Card>,
    }

    impl TestState {
        fn two_players() -> Self {
            Self {
                state_type: StateType::Turn { player: 0 },
                player_coins: vec![2, 2],
                player_hands: vec![2, 2],
                player_cards_counter: vec![2, 2],
                player_cards: vec![
                    vec![Card::Assassin, Card::Captain],
                    vec![Card::Ambassador, Card::Duke],
                ],
                deck: vec![Card::Contessa],
                revealed_cards: Vec::with_capacity(5),
            }
        }

        fn four_players() -> Self {
            Self {
                state_type: StateType::Turn { player: 0 },
                player_coins: vec![2, 2, 2, 2],
                player_hands: vec![2, 2, 1, 0],
                player_cards_counter: vec![2, 2, 1, 0],
                player_cards: vec![
                    vec![Card::Assassin, Card::Captain],
                    vec![Card::Ambassador, Card::Duke],
                    vec![Card::Contessa, Card::Assassin],
                    vec![Card::Captain, Card::Ambassador],
                ],
                deck: vec![
                    Card::Duke,
                    Card::Contessa,
                ],
                revealed_cards: Vec::with_capacity(2 * 5),
            }
        }

        fn state(&mut self) -> State<Vec<Card>, Vec<Card>> {
            State {
                state_type: &mut self.state_type,
                player_coins: &mut self.player_coins,
                player_hands: &mut self.player_hands,
                player_cards_counter: &mut self.player_cards_counter,
                player_cards: &mut self.player_cards,
                deck: &mut self.deck,
                revealed_cards: &mut self.revealed_cards,
            }
        }
    }

    #[test]
    fn income_for_turn_should_return_turn_for_next_player() {
        let mut state = TestState::two_players();
        assert_eq!(
            play_action(
                &Action { player: 0, action_type: ActionType::Income },
                &mut state.state(),
                &mut ConstRng,
            ),
            Ok(()),
        );
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
        assert_eq!(state.player_coins[0], 3);
    }

    #[test]
    fn foreign_aid_for_turn_should_return_foreign_aid() {
        let mut state = TestState::two_players();
        assert_eq!(
            play_action(
                &Action { player: 0, action_type: ActionType::ForeignAid },
                &mut state.state(),
                &mut ConstRng,
            ),
            Ok(()),
        );
        assert_eq!(state.state_type, StateType::ForeignAid { player: 0 });
    }

    #[test]
    fn tax_for_turn_should_return_tax() {
        let mut state = TestState::two_players();
        assert_eq!(
            play_action(
                &Action { player: 0, action_type: ActionType::ForeignAid },
                &mut state.state(),
                &mut ConstRng,
            ),
            Ok(()),
        );
        assert_eq!(state.state_type, StateType::ForeignAid { player: 0 });
    }

    #[test]
    fn assassinate_for_turn_should_return_assassination() {
        let mut state = TestState::two_players();
        state.player_coins[0] = 3;
        assert_eq!(
            play_action(
                &Action { player: 0, action_type: ActionType::Assassinate(1) },
                &mut state.state(),
                &mut ConstRng,
            ),
            Ok(()),
        );
        assert_eq!(state.state_type, StateType::Assassination { player: 0, target: 1, can_challenge: true });
        assert_eq!(state.player_coins, vec![0, 2]);
    }

    #[test]
    fn steal_for_turn_should_return_steal() {
        let mut state = TestState::two_players();
        assert_eq!(
            play_action(
                &Action { player: 0, action_type: ActionType::Steal(1) },
                &mut state.state(),
                &mut ConstRng,
            ),
            Ok(()),
        );
        assert_eq!(state.state_type, StateType::Steal { player: 0, target: 1, can_challenge: true });
    }

    #[test]
    fn coup_for_turn_should_return_lost_influence() {
        let mut state = TestState::two_players();
        state.player_coins[0] = 7;
        assert_eq!(
            play_action(
                &Action { player: 0, action_type: ActionType::Coup(1) },
                &mut state.state(),
                &mut ConstRng,
            ),
            Ok(()),
        );
        assert_eq!(state.state_type, StateType::LostInfluence { player: 1, current_player: 0 });
        assert_eq!(state.player_coins[0], 0);
    }

    #[test]
    fn exchange_for_turn_should_return_exchange() {
        let mut state = TestState::two_players();
        assert_eq!(
            play_action(
                &Action { player: 0, action_type: ActionType::Exchange },
                &mut state.state(),
                &mut ConstRng,
            ),
            Ok(()),
        );
        assert_eq!(state.state_type, StateType::Exchange { player: 0 });
    }

    #[test]
    fn reveal_card_for_lost_influence_should_return_turn_for_next_player() {
        let mut state = TestState::two_players();
        state.state_type = StateType::LostInfluence { player: 1, current_player: 0 };
        assert_eq!(
            play_action(
                &Action { player: 1, action_type: ActionType::RevealCard(Card::Ambassador) },
                &mut state.state(),
                &mut ConstRng,
            ),
            Ok(()),
        );
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
    }

    #[test]
    fn pass_block_for_turn_should_return_invalid_action_error() {
        let mut state = TestState::two_players();
        assert_eq!(
            play_action(
                &Action { player: 0, action_type: ActionType::PassBlock },
                &mut state.state(),
                &mut ConstRng,
            ),
            Err(Error::InvalidAction),
        );
        assert_eq!(state.state_type, StateType::Turn { player: 0 });
    }

    #[test]
    fn block_foreign_aid_for_tax_should_return_invalid_action_error() {
        let mut state = TestState::two_players();
        state.state_type = StateType::Tax { player: 0 };
        assert_eq!(
            play_action(
                &Action { player: 0, action_type: ActionType::BlockForeignAid },
                &mut state.state(),
                &mut ConstRng,
            ),
            Err(Error::InvalidAction),
        );
        assert_eq!(state.state_type, StateType::Tax { player: 0 });
    }

    #[test]
    fn successfully_blocked_foreign_aid_leads_to_next_turn() {
        let mut state = TestState::four_players();
        let actions = [
            Action { player: 0, action_type: ActionType::ForeignAid },
            Action { player: 1, action_type: ActionType::BlockForeignAid },
            Action { player: 1, action_type: ActionType::PassChallenge },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
    }

    #[test]
    fn successfully_challenged_blocked_foreign_aid_leads_to_next_turn() {
        let mut state = TestState::four_players();
        let actions = [
            Action { player: 0, action_type: ActionType::ForeignAid },
            Action { player: 1, action_type: ActionType::BlockForeignAid },
            Action { player: 2, action_type: ActionType::Challenge },
            Action { player: 1, action_type: ActionType::RevealCard(Card::Duke) },
            Action { player: 0, action_type: ActionType::PassBlock },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
        assert_eq!(state.player_coins, vec![4, 2, 2, 2]);
    }

    #[test]
    fn block_foreign_aid_can_be_successfully_challenged_multiple_times() {
        let mut state = TestState::four_players();
        let actions = [
            Action { player: 0, action_type: ActionType::ForeignAid },
            Action { player: 1, action_type: ActionType::BlockForeignAid },
            Action { player: 2, action_type: ActionType::Challenge },
            Action { player: 1, action_type: ActionType::RevealCard(Card::Ambassador) },
            Action { player: 2, action_type: ActionType::BlockForeignAid },
            Action { player: 0, action_type: ActionType::Challenge },
            Action { player: 2, action_type: ActionType::RevealCard(Card::Contessa) },
            Action { player: 0, action_type: ActionType::PassBlock },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
        assert_eq!(state.player_coins, vec![4, 2, 2, 2]);
    }

    #[test]
    fn failed_on_challenge_blocked_foreign_aid_leads_to_next_turn() {
        let mut state = TestState::four_players();
        let actions = [
            Action { player: 0, action_type: ActionType::ForeignAid },
            Action { player: 1, action_type: ActionType::BlockForeignAid },
            Action { player: 2, action_type: ActionType::Challenge },
            Action { player: 1, action_type: ActionType::ShowCard(Card::Duke) },
            Action { player: 2, action_type: ActionType::RevealCard(Card::Contessa) },
            Action { player: 1, action_type: ActionType::ShuffleDeck },
            Action { player: 1, action_type: ActionType::TakeCard },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
        assert_eq!(state.player_coins, vec![2, 2, 2, 2]);
    }

    #[test]
    fn unchallenged_tax_leads_to_next_turn() {
        let mut state = TestState::four_players();
        let actions = [
            Action { player: 0, action_type: ActionType::Tax },
            Action { player: 0, action_type: ActionType::PassChallenge },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
        assert_eq!(state.player_coins, vec![5, 2, 2, 2]);
    }

    #[test]
    fn successfully_challenged_tax_leads_to_next_turn() {
        let mut state = TestState::four_players();
        let actions = [
            Action { player: 0, action_type: ActionType::Tax },
            Action { player: 1, action_type: ActionType::Challenge },
            Action { player: 0, action_type: ActionType::RevealCard(Card::Assassin) },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
    }

    #[test]
    fn failed_tax_challenge_leads_to_next_turn() {
        let mut state = TestState::four_players();
        state.state_type = StateType::Turn { player: 1 };
        let actions = [
            Action { player: 1, action_type: ActionType::Tax },
            Action { player: 0, action_type: ActionType::Challenge },
            Action { player: 1, action_type: ActionType::ShowCard(Card::Duke) },
            Action { player: 0, action_type: ActionType::RevealCard(Card::Assassin) },
            Action { player: 1, action_type: ActionType::ShuffleDeck },
            Action { player: 1, action_type: ActionType::TakeCard },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 2 });
        assert_eq!(state.player_coins, vec![2, 5, 2, 2]);
    }

    #[test]
    fn block_assassination_can_be_challenged_by_any_player() {
        let mut state = TestState::four_players();
        state.player_coins[0] = 3;
        let actions = [
            Action { player: 0, action_type: ActionType::Assassinate(2) },
            Action { player: 0, action_type: ActionType::PassChallenge },
            Action { player: 2, action_type: ActionType::BlockAssassination },
            Action { player: 1, action_type: ActionType::Challenge },
            Action { player: 2, action_type: ActionType::ShowCard(Card::Contessa) },
            Action { player: 1, action_type: ActionType::RevealCard(Card::Ambassador) },
            Action { player: 2, action_type: ActionType::ShuffleDeck },
            Action { player: 2, action_type: ActionType::TakeCard },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
        assert_eq!(state.player_coins, vec![0, 2, 2, 2]);
    }

    #[test]
    fn successful_assassination_leads_to_next_turn_when_target_has_only_one_card_in_hand() {
        let mut state = TestState::four_players();
        state.player_coins[0] = 3;
        let actions = [
            Action { player: 0, action_type: ActionType::Assassinate(2) },
            Action { player: 1, action_type: ActionType::Challenge },
            Action { player: 0, action_type: ActionType::ShowCard(Card::Assassin) },
            Action { player: 1, action_type: ActionType::RevealCard(Card::Ambassador) },
            Action { player: 0, action_type: ActionType::ShuffleDeck },
            Action { player: 0, action_type: ActionType::TakeCard },
            Action { player: 2, action_type: ActionType::BlockAssassination },
            Action { player: 0, action_type: ActionType::Challenge },
            Action { player: 2, action_type: ActionType::RevealCard(Card::Contessa) },
            Action { player: 0, action_type: ActionType::PassBlock },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
        assert_eq!(state.player_coins, vec![0, 2, 2, 2]);
    }

    #[test]
    fn steal_can_be_challenged_by_any_and_blocked_by_target_player() {
        let mut state = TestState::four_players();
        let actions = [
            Action { player: 0, action_type: ActionType::Steal(1) },
            Action { player: 2, action_type: ActionType::Challenge },
            Action { player: 0, action_type: ActionType::ShowCard(Card::Captain) },
            Action { player: 2, action_type: ActionType::RevealCard(Card::Contessa) },
            Action { player: 0, action_type: ActionType::ShuffleDeck },
            Action { player: 0, action_type: ActionType::TakeCard },
            Action { player: 1, action_type: ActionType::BlockSteal(Card::Ambassador) },
            Action { player: 1, action_type: ActionType::PassChallenge },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
        assert_eq!(state.player_coins, vec![2, 2, 2, 2]);
    }

    #[test]
    fn successfully_challenged_steal_leads_to_next_turn() {
        let mut state = TestState::four_players();
        let actions = [
            Action { player: 0, action_type: ActionType::Steal(1) },
            Action { player: 2, action_type: ActionType::Challenge },
            Action { player: 0, action_type: ActionType::RevealCard(Card::Assassin) },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
        assert_eq!(state.player_coins, vec![2, 2, 2, 2]);
    }

    #[test]
    fn successful_steal_leads_to_next_turn() {
        let mut state = TestState::four_players();
        let actions = [
            Action { player: 0, action_type: ActionType::Steal(1) },
            Action { player: 1, action_type: ActionType::Challenge },
            Action { player: 0, action_type: ActionType::ShowCard(Card::Captain) },
            Action { player: 1, action_type: ActionType::RevealCard(Card::Ambassador) },
            Action { player: 0, action_type: ActionType::ShuffleDeck },
            Action { player: 0, action_type: ActionType::TakeCard },
            Action { player: 1, action_type: ActionType::BlockSteal(Card::Captain) },
            Action { player: 0, action_type: ActionType::Challenge },
            Action { player: 1, action_type: ActionType::RevealCard(Card::Duke) },
            Action { player: 0, action_type: ActionType::PassBlock },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 2 });
        assert_eq!(state.player_coins, vec![4, 0, 2, 2]);
    }

    #[test]
    fn block_steal_can_be_challenged_by_any_player() {
        let mut state = TestState::four_players();
        let actions = [
            Action { player: 0, action_type: ActionType::Steal(1) },
            Action { player: 0, action_type: ActionType::PassChallenge },
            Action { player: 1, action_type: ActionType::BlockSteal(Card::Captain) },
            Action { player: 2, action_type: ActionType::Challenge },
            Action { player: 1, action_type: ActionType::RevealCard(Card::Duke) },
            Action { player: 1, action_type: ActionType::BlockSteal(Card::Ambassador) },
            Action { player: 0, action_type: ActionType::Challenge },
            Action { player: 1, action_type: ActionType::ShowCard(Card::Ambassador) },
            Action { player: 0, action_type: ActionType::RevealCard(Card::Assassin) },
            Action { player: 1, action_type: ActionType::ShuffleDeck },
            Action { player: 1, action_type: ActionType::TakeCard },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 1 });
        assert_eq!(state.player_coins, vec![2, 2, 2, 2]);
    }

    #[test]
    fn successful_exchange_requires_to_drop_cards() {
        let mut state = TestState::four_players();
        state.state_type = StateType::Turn { player: 1 };
        let actions = [
            Action { player: 1, action_type: ActionType::Exchange },
            Action { player: 2, action_type: ActionType::Challenge },
            Action { player: 1, action_type: ActionType::ShowCard(Card::Ambassador) },
            Action { player: 2, action_type: ActionType::RevealCard(Card::Contessa) },
            Action { player: 1, action_type: ActionType::ShuffleDeck },
            Action { player: 1, action_type: ActionType::TakeCard },
            Action { player: 1, action_type: ActionType::TakeCard },
            Action { player: 1, action_type: ActionType::TakeCard },
            Action { player: 1, action_type: ActionType::DropCard(Card::Contessa) },
            Action { player: 1, action_type: ActionType::DropCard(Card::Duke) },
        ];
        assert_eq!(play_actions(&mut state, &actions), Ok(()));
        assert_eq!(state.state_type, StateType::Turn { player: 0 });
    }

    fn play_actions(state: &mut TestState, actions: &[Action]) -> Result<(), Error> {
        for action in actions {
            println!("Play action={:?} for state={:?}", action, state);
            match play_action(action, &mut state.state(), &mut ConstRng) {
                Ok(..) => (),
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}
