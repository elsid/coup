use itertools::Itertools;
use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Card {
    Assassin,
    Ambassador,
    Captain,
    Contessa,
    Duke,
}

pub const ALL_CARDS: [Card; 5] = [Card::Assassin, Card::Ambassador, Card::Captain, Card::Contessa, Card::Duke];
pub const CARDS_PER_PLAYER: usize = 2;
pub const MAX_CARDS_TO_EXCHANGE: usize = 2;
const STEAL_BLOCKERS: [Card; CARDS_PER_PLAYER] = [Card::Ambassador, Card::Captain];
const ASSASSINATION_COST: usize = 3;
const COUP_COST: usize = 7;
const MAX_COINS: usize = 10;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
struct Player {
    coins: usize,
    cards: Vec<PlayerCard>,
}

impl Player {
    fn is_active(&self) -> bool {
        self.cards.iter().any(|v| !v.revealed)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpponentView {
    pub coins: usize,
    pub hand: usize,
    pub revealed_cards: Vec<Card>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct PlayerView<'a> {
    pub step: usize,
    pub turn: usize,
    pub round: usize,
    pub game_player: usize,
    pub player: usize,
    pub coins: usize,
    pub cards: &'a [PlayerCard],
    pub players: Vec<OpponentView>,
    pub blockers: &'a [Blocker],
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct AnonymousView<'a> {
    pub player: usize,
    pub players: Vec<OpponentView>,
    pub blockers: &'a [Blocker],
}

pub fn get_available_actions(player: usize, players: &[OpponentView], blockers: &[Blocker]) -> Vec<Action> {
    let mut actions: Vec<Action> = Vec::new();
    if blockers.is_empty() {
        if players[player].coins >= MAX_COINS {
            for i in 0..players.len() {
                if i != player && players[i].hand > 0 {
                    actions.push(Action { player, action_type: ActionType::Coup(i) });
                }
            }
        } else {
            let action_types = [ActionType::Income, ActionType::ForeignAid, ActionType::Tax, ActionType::Exchange];
            for action_type in action_types.iter().cloned() {
                actions.push(Action { player, action_type });
            }
            for i in 0..players.len() {
                if i != player && players[i].hand > 0 {
                    actions.push(Action { player, action_type: ActionType::Steal(i) });
                    if players[player].coins >= ASSASSINATION_COST {
                        actions.push(Action { player, action_type: ActionType::Assassinate(i) });
                    }
                    if players[player].coins >= COUP_COST {
                        actions.push(Action { player, action_type: ActionType::Coup(i) });
                    }
                }
            }
        }
    } else {
        match blockers.last().unwrap() {
            Blocker::Counteraction { action_type, target, .. } => {
                match action_type {
                    ActionType::ForeignAid => {
                        for i in 0..players.len() {
                            if i != *target && players[i].hand > 0 {
                                actions.push(Action {
                                    player: i,
                                    action_type: ActionType::BlockForeignAid,
                                });
                            }
                        }
                        actions.push(Action {
                            player: *target,
                            action_type: ActionType::Complete,
                        });
                    }
                    ActionType::Assassinate(..) => {
                        for i in 0..players.len() {
                            if i != *target && players[i].hand > 0 {
                                actions.push(Action {
                                    player: i,
                                    action_type: ActionType::BlockAssassination,
                                });
                                actions.push(Action {
                                    player: i,
                                    action_type: ActionType::Challenge,
                                });
                            }
                        }
                        actions.push(Action {
                            player: *target,
                            action_type: ActionType::Complete,
                        });
                    }
                    ActionType::Steal(..) => {
                        for i in 0..players.len() {
                            if i != *target && players[i].hand > 0 {
                                for card in &STEAL_BLOCKERS {
                                    actions.push(Action {
                                        player: i,
                                        action_type: ActionType::BlockSteal(*card),
                                    });
                                }
                                actions.push(Action {
                                    player: i,
                                    action_type: ActionType::Challenge,
                                });
                            }
                        }
                        actions.push(Action {
                            player: *target,
                            action_type: ActionType::Complete,
                        });
                    }
                    ActionType::Tax | ActionType::Exchange | ActionType::BlockForeignAid
                    | ActionType::BlockAssassination | ActionType::BlockSteal(..) => {
                        for i in 0..players.len() {
                            if i != *target && players[i].hand > 0 {
                                actions.push(Action {
                                    player: i,
                                    action_type: ActionType::Challenge,
                                });
                            }
                        }
                        actions.push(Action {
                            player: *target,
                            action_type: ActionType::Complete,
                        });
                    }
                    _ => (),
                }
            }
            Blocker::Challenge { card, target, .. } => {
                actions.push(Action {
                    player: *target,
                    action_type: ActionType::ShowCard(*card),
                });
                for card in &ALL_CARDS {
                    actions.push(Action {
                        player: *target,
                        action_type: ActionType::RevealCard(*card),
                    });
                }
            }
            Blocker::RevealCard { target } => {
                for card in &ALL_CARDS {
                    actions.push(Action {
                        player: *target,
                        action_type: ActionType::RevealCard(*card),
                    });
                }
            }
            Blocker::DropCard { target } => {
                for card in &ALL_CARDS {
                    actions.push(Action {
                        player: *target,
                        action_type: ActionType::DropCard(*card),
                    });
                }
            }
        }
    }
    actions
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct PlayerCard {
    pub kind: Card,
    pub revealed: bool,
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
    players: Vec<Player>,
    deck: Vec<Card>,
    player: usize,
    blockers: Vec<Blocker>,
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
    Complete,
    Challenge,
    ShowCard(Card),
    RevealCard(Card),
    DropCard(Card),
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Blocker {
    Counteraction {
        action_type: ActionType,
        target: usize,
        source: Option<usize>,
    },
    Challenge {
        target: usize,
        source: usize,
        card: Card,
    },
    RevealCard {
        target: usize,
    },
    DropCard {
        target: usize,
    },
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
        let mut players: Vec<Player> = std::iter::repeat(Player {
            coins: 2,
            cards: Vec::new(),
        }).take(settings.players_number).collect();
        for _ in 0..CARDS_PER_PLAYER {
            for player in players.iter_mut() {
                player.cards.push(PlayerCard {
                    kind: deck.pop().unwrap(),
                    revealed: false,
                });
            }
        }
        Self {
            step: 0,
            turn: 0,
            round: 0,
            players,
            deck,
            player: 0,
            blockers: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn custom(players: Vec<Vec<Card>>, deck: Vec<Card>) -> Self {
        Self {
            step: 0,
            turn: 0,
            round: 0,
            players: players.into_iter()
                .map(|cards| Player {
                    coins: 2,
                    cards: cards.into_iter().map(|kind| PlayerCard { kind, revealed: false }).collect(),
                })
                .collect(),
            deck,
            player: 0,
            blockers: Vec::new(),
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
            player: self.player,
            players: self.players.iter()
                .map(|player| OpponentView {
                    coins: player.coins,
                    hand: player.cards.iter()
                        .filter(|card| !card.revealed)
                        .count(),
                    revealed_cards: player.cards.iter()
                        .filter(|card| card.revealed)
                        .map(|card| card.kind)
                        .collect(),
                })
                .collect(),
            blockers: &self.blockers,
        }
    }

    pub fn get_player_view(&self, index: usize) -> PlayerView {
        let player = &self.players[index];
        PlayerView {
            step: self.step,
            turn: self.turn,
            round: self.round,
            game_player: self.player,
            player: index,
            coins: player.coins,
            cards: &player.cards,
            players: self.players.iter()
                .map(|player| OpponentView {
                    coins: player.coins,
                    hand: player.cards.iter()
                        .filter(|card| !card.revealed)
                        .count(),
                    revealed_cards: player.cards.iter()
                        .filter(|card| card.revealed)
                        .map(|card| card.kind)
                        .collect(),
                })
                .collect(),
            blockers: &self.blockers,
        }
    }

    pub fn is_player_active(&self, index: usize) -> bool {
        self.players[index].is_active()
    }

    pub fn is_done(&self) -> bool {
        self.players.iter()
            .filter(|player| player.cards.iter().any(|card| !card.revealed))
            .count() <= 1
    }

    pub fn get_winner(&self) -> Option<usize> {
        if self.is_done() {
            self.players.iter()
                .find_position(|player| player.cards.iter().any(|card| !card.revealed))
                .map(|(index, _)| index)
        } else {
            None
        }
    }

    pub fn play<R: Rng>(&mut self, action: &Action, rng: &mut R) -> Result<(), String> {
        let player = self.player;
        let result = match &action.action_type {
            ActionType::Income => self.income(action.player),
            ActionType::ForeignAid | ActionType::Tax | ActionType::Exchange => self.action_without_target(&action.action_type, action.player),
            ActionType::Coup(target) => self.coup(action.player, *target),
            ActionType::Assassinate(target) => self.assassinate(action.player, *target),
            ActionType::Steal(target) => self.steal(action.player, *target),
            ActionType::BlockForeignAid => self.block_foreign_aid(action.player),
            ActionType::BlockSteal(card) => self.block_steal(action.player, *card),
            ActionType::BlockAssassination => self.block_assassination(action.player),
            ActionType::Complete => self.complete(action.player),
            ActionType::Challenge => self.challenge(action.player),
            ActionType::ShowCard(card) => self.show_card(action.player, *card, rng),
            ActionType::RevealCard(card) => self.reveal_card(action.player, *card),
            ActionType::DropCard(card) => self.drop_card(action.player, *card),
        };
        if matches!(result, Ok(..)) {
            self.step += 1;
            if player != self.player {
                self.turn += 1;
                if player > self.player {
                    self.round += 1;
                }
            }
        }
        result
    }

    fn income(&mut self, player: usize) -> Result<(), String> {
        if player != self.player {
            return Err(format!("Require action player: {}", self.player));
        }
        if !self.blockers.is_empty() {
            return Err(String::from("Require empty blockers"));
        }
        if self.players[player].coins >= MAX_COINS {
            return Err(format!("Require to do coup with {} coins", MAX_COINS));
        }
        self.players[player].coins += 1;
        self.advance_player();
        Ok(())
    }

    fn action_without_target(&mut self, action_type: &ActionType, player: usize) -> Result<(), String> {
        if player != self.player {
            return Err(format!("Require action player: {}", self.player));
        }
        if !self.blockers.is_empty() {
            return Err(String::from("Require empty blockers"));
        }
        if self.players[player].coins >= MAX_COINS {
            return Err(format!("Require to do coup with {} coins", MAX_COINS));
        }
        self.blockers.push(Blocker::Counteraction {
            action_type: action_type.clone(),
            target: player,
            source: None,
        });
        Ok(())
    }

    fn coup(&mut self, player: usize, target: usize) -> Result<(), String> {
        if player != self.player {
            return Err(format!("Require action player: {}", self.player));
        }
        if !self.blockers.is_empty() {
            return Err(String::from("Require empty blockers"));
        }
        if player == target {
            return Err(format!("Require to coup other player"));
        }
        if self.players[player].coins < COUP_COST {
            return Err(format!("Require {} coins for coup: {}", COUP_COST, self.players[player].coins));
        }
        if !self.players[target].is_active() {
            return Err(format!("Require active target player"));
        }
        self.players[player].coins -= COUP_COST;
        self.blockers.push(Blocker::RevealCard { target });
        Ok(())
    }

    fn assassinate(&mut self, player: usize, target: usize) -> Result<(), String> {
        if player != self.player {
            return Err(format!("Require action player: {}", self.player));
        }
        if !self.blockers.is_empty() {
            return Err(String::from("Require empty blockers"));
        }
        if player == target {
            return Err(format!("Require other player target"));
        }
        if !self.players[target].is_active() {
            return Err(format!("Require active target player"));
        }
        if self.players[player].coins < ASSASSINATION_COST {
            return Err(format!("Require {} coins: {}", ASSASSINATION_COST, self.players[player].coins));
        }
        if self.players[player].coins >= MAX_COINS {
            return Err(format!("Require to do coup with {} coins", MAX_COINS));
        }
        self.players[player].coins -= ASSASSINATION_COST;
        self.blockers.push(Blocker::Counteraction {
            action_type: ActionType::Assassinate(target),
            target: player,
            source: None,
        });
        Ok(())
    }

    fn steal(&mut self, player: usize, target: usize) -> Result<(), String> {
        if player != self.player {
            return Err(format!("Require action player: {}", self.player));
        }
        if !self.blockers.is_empty() {
            return Err(String::from("Require empty blockers"));
        }
        if player == target {
            return Err(String::from("Require other player target"));
        }
        if !self.players[target].is_active() {
            return Err(String::from("Require active target player"));
        }
        if self.players[player].coins >= MAX_COINS {
            return Err(format!("Require to do coup with {} coins", MAX_COINS));
        }
        self.blockers.push(Blocker::Counteraction {
            action_type: ActionType::Steal(target),
            target: player,
            source: None,
        });
        Ok(())
    }

    fn block_foreign_aid(&mut self, player: usize) -> Result<(), String> {
        if player == self.player {
            return Err(format!("Require action for not player: {}", self.player));
        }
        if let Some(Blocker::Counteraction { source, .. }) = self.blockers.last_mut() {
            if source.is_some() {
                return Err(String::from("Require counteraction without source"));
            }
            *source = Some(player);
        } else {
            return Err(String::from("Require counteraction last blocker"));
        }
        self.blockers.push(Blocker::Counteraction {
            action_type: ActionType::BlockForeignAid,
            target: player,
            source: None,
        });
        Ok(())
    }

    fn block_steal(&mut self, player: usize, card: Card) -> Result<(), String> {
        if player == self.player {
            return Err(format!("Require action for not player: {}", self.player));
        }
        if !matches!(card, Card::Ambassador | Card::Captain) {
            return Err(format!("Require ambassador or captain card: {:?}", card));
        }
        if let Some(Blocker::Counteraction { source, .. }) = self.blockers.last_mut() {
            if source.is_some() {
                return Err(String::from("Require counteraction without source"));
            }
            *source = Some(player);
        } else {
            return Err(String::from("Require counteraction last blocker"));
        };
        self.blockers.push(Blocker::Counteraction {
            action_type: ActionType::BlockSteal(card),
            target: player,
            source: None,
        });
        Ok(())
    }

    fn block_assassination(&mut self, player: usize) -> Result<(), String> {
        if player == self.player {
            return Err(format!("Require action for not player: {}", self.player));
        }
        if let Some(Blocker::Counteraction { source, .. }) = self.blockers.last_mut() {
            if source.is_some() {
                return Err(String::from("Require counteraction without source"));
            }
            *source = Some(player);
        } else {
            return Err(String::from("Require counteraction last blocker"));
        };
        self.blockers.push(Blocker::Counteraction {
            action_type: ActionType::BlockAssassination,
            target: player,
            source: None,
        });
        Ok(())
    }

    fn complete(&mut self, player: usize) -> Result<(), String> {
        if self.blockers.len() < 1 {
            return Err(String::from("Require at least one blocker"));
        }
        let (action_type, target) = if let Some(Blocker::Counteraction { action_type, source: None, target }) = self.blockers.last() {
            if player != *target {
                return Err(format!("Require action player matching counteraction target"));
            }
            (action_type.clone(), *target)
        } else {
            return Err(format!("Require counteraction last blocker without source: {:?}", self.blockers));
        };
        self.blockers.pop();
        while !self.blockers.is_empty() {
            self.blockers.pop();
        }
        self.complete_action(&action_type, target);
        if self.blockers.is_empty() {
            self.advance_player();
        }
        Ok(())
    }

    fn challenge(&mut self, player: usize) -> Result<(), String> {
        let (card, target) = if let Some(Blocker::Counteraction { action_type, source, target, .. }) = self.blockers.last_mut() {
            let card = match action_type {
                ActionType::BlockForeignAid | ActionType::Tax => Card::Duke,
                ActionType::BlockSteal(card) => *card,
                ActionType::BlockAssassination => Card::Contessa,
                ActionType::Steal(..) => Card::Captain,
                ActionType::Assassinate(..) => Card::Assassin,
                ActionType::Exchange => Card::Ambassador,
                _ => return Err(format!("Card is not defined for action type: {:?}", action_type)),
            };
            if source.is_none() {
                *source = Some(player);
                (card, *target)
            } else if *target != player {
                return Err(format!("Require challenge by player: {}", target));
            } else {
                (card, source.unwrap())
            }
        } else {
            return Err(String::from("Require counteraction last blocker"));
        };
        self.blockers.push(Blocker::Challenge {
            target,
            source: player,
            card,
        });
        Ok(())
    }

    fn show_card<R: Rng>(&mut self, player: usize, shown_card: Card, rng: &mut R) -> Result<(), String> {
        let (card_index, winner, loser) = if let Some(Blocker::Challenge { card, target, source }) = self.blockers.last() {
            if shown_card != *card {
                return Err(format!("Require shown card: {:?}, got: {:?}", card, shown_card));
            }
            if *target != player {
                return Err(format!("Require action player: {}", target));
            }
            if let Some((card_index, _)) = self.players[*target].cards.iter()
                .find_position(|v| !v.revealed && v.kind == *card) {
                (card_index, *target, *source)
            } else {
                return Err(format!("Require player to have non revealed card: {:?}", shown_card));
            }
        } else {
            return Err(format!("Require challenge last blocker: {:?}", self.blockers));
        };
        self.players[winner].cards.remove(card_index);
        self.deck.push(shown_card);
        self.deck.shuffle(rng);
        self.players[winner].cards.push(PlayerCard {
            kind: self.deck.pop().unwrap(),
            revealed: false,
        });
        *self.blockers.last_mut().unwrap() = Blocker::RevealCard { target: loser };
        Ok(())
    }

    fn reveal_card(&mut self, player: usize, card: Card) -> Result<(), String> {
        if let Some(Blocker::RevealCard { target }) = self.blockers.last() {
            if *target != player {
                return Err(format!("Require action player: {}", target));
            }
        } else if let Some(Blocker::Challenge { target, .. }) = self.blockers.last() {
            if *target != player {
                return Err(format!("Require action player: {}", target));
            }
        } else {
            return Err(format!("Require reveal card or challenge blocker"));
        }
        if let Some(player_card) = self.players[player].cards.iter_mut()
            .find(|v| !v.revealed && v.kind == card) {
            player_card.revealed = true;
        } else {
            return Err(format!("Require player to have non revealed card: {:?}", card));
        }
        self.blockers.pop();
        while !self.blockers.is_empty() {
            let (action_type, target) = if let Some(Blocker::Counteraction { action_type, target, .. }) = self.blockers.last() {
                (action_type.clone(), *target)
            } else {
                break;
            };
            self.blockers.pop();
            if player != target {
                self.complete_action(&action_type, target);
            }
        }
        if self.blockers.is_empty() {
            self.advance_player();
        }
        Ok(())
    }

    fn drop_card(&mut self, player: usize, card: Card) -> Result<(), String> {
        if let Some(Blocker::DropCard { target }) = self.blockers.last() {
            if *target != player {
                return Err(format!("Require action player: {}", target));
            }
        } else {
            return Err(format!("Require drop card blocker"));
        }
        let card_index = if let Some((card_index, _)) = self.players[player].cards.iter_mut()
            .find_position(|v| !v.revealed && v.kind == card) {
            card_index
        } else {
            return Err(format!("Require player to have non revealed card: {:?}", card));
        };
        self.blockers.pop();
        let card = self.players[player].cards.remove(card_index).kind;
        self.deck.push(card);
        if self.blockers.is_empty() {
            self.advance_player();
        }
        Ok(())
    }

    fn complete_action(&mut self, action_type: &ActionType, player: usize) {
        match action_type {
            ActionType::ForeignAid => self.players[player].coins += 2,
            ActionType::Tax => self.players[player].coins += 3,
            ActionType::Assassinate(target) => {
                if self.players[*target].is_active() {
                    self.blockers.push(Blocker::RevealCard { target: *target });
                }
            }
            ActionType::Exchange => {
                for _ in 0..self.deck.len().min(MAX_CARDS_TO_EXCHANGE) {
                    self.players[player].cards.push(PlayerCard {
                        kind: self.deck.pop().unwrap(),
                        revealed: false,
                    });
                    self.blockers.push(Blocker::DropCard {
                        target: player,
                    });
                }
            }
            ActionType::Steal(target) => {
                self.players[player].coins += self.players[*target].coins.min(2);
                self.players[*target].coins -= self.players[*target].coins.min(2);
            }
            _ => (),
        }
    }

    fn advance_player(&mut self) {
        while !self.players[(self.player + 1) % self.players.len()].is_active() {
            self.player += 1;
        }
        self.player = (self.player + 1) % self.players.len();
    }

    pub fn print(&self) {
        println!("Round: {}, turn: {}, step: {}", self.round, self.turn, self.step);
        println!("Done: {}", self.is_done());
        println!("Deck: {}", self.deck.len());
        for i in 0..self.deck.len() {
            println!("    {}) {:?}", i, self.deck[i]);
        }
        let winner = self.get_winner();
        println!("Players: {}", self.players.len());
        for i in 0..self.players.len() {
            let player = &self.players[i];
            if winner == Some(i) {
                print!("W");
            } else {
                print!(" ");
            }
            if i == self.player {
                print!("-> ");
            } else {
                print!("   ");
            }
            print!("{})", i);
            if player.is_active() {
                print!(" + ");
            } else {
                print!(" - ");
            }
            println!("{:?}", player);
        }
        println!("Blockers: {}", self.blockers.len());
        for i in 0..self.blockers.len() {
            println!("    {}) {:?}", i, self.blockers[i]);
        }
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
            action_type: ActionType::Complete,
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
            player: 3,
            action_type: ActionType::Tax,
        },
        Action {
            player: 3,
            action_type: ActionType::Complete,
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
            player: 3,
            action_type: ActionType::BlockSteal(Card::Ambassador),
        },
        Action {
            player: 3,
            action_type: ActionType::Complete,
        },
        Action {
            player: 0,
            action_type: ActionType::Tax,
        },
        Action {
            player: 0,
            action_type: ActionType::Complete,
        },
        Action {
            player: 1,
            action_type: ActionType::Steal(3),
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
            player: 2,
            action_type: ActionType::Tax,
        },
        Action {
            player: 2,
            action_type: ActionType::Complete,
        },
        Action {
            player: 3,
            action_type: ActionType::Steal(2),
        },
        Action {
            player: 3,
            action_type: ActionType::Complete,
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
            action_type: ActionType::Complete,
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
            action_type: ActionType::Complete,
        },
        Action {
            player: 3,
            action_type: ActionType::Steal(2),
        },
        Action {
            player: 3,
            action_type: ActionType::Complete,
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
            action_type: ActionType::Complete,
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
            action_type: ActionType::Complete,
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
            player: 1,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 1,
            action_type: ActionType::Complete,
        },
        Action {
            player: 3,
            action_type: ActionType::Steal(4),
        },
        Action {
            player: 3,
            action_type: ActionType::Complete,
        },
        Action {
            player: 4,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 4,
            action_type: ActionType::Complete,
        },
        Action {
            player: 5,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 5,
            action_type: ActionType::Complete,
        },
        Action {
            player: 0,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 0,
            action_type: ActionType::Complete,
        },
        Action {
            player: 1,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 1,
            action_type: ActionType::Complete,
        },
        Action {
            player: 3,
            action_type: ActionType::Steal(5),
        },
        Action {
            player: 3,
            action_type: ActionType::Complete,
        },
        Action {
            player: 4,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 4,
            action_type: ActionType::Complete,
        },
        Action {
            player: 5,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 5,
            action_type: ActionType::Complete,
        },
        Action {
            player: 0,
            action_type: ActionType::Assassinate(1),
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
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 1,
            action_type: ActionType::Complete,
        },
        Action {
            player: 3,
            action_type: ActionType::Steal(1),
        },
        Action {
            player: 3,
            action_type: ActionType::Complete,
        },
        Action {
            player: 4,
            action_type: ActionType::Exchange,
        },
        Action {
            player: 4,
            action_type: ActionType::Complete,
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
            player: 1,
            action_type: ActionType::Tax,
        },
        Action {
            player: 1,
            action_type: ActionType::Complete,
        },
        Action {
            player: 3,
            action_type: ActionType::Steal(1),
        },
        Action {
            player: 3,
            action_type: ActionType::Complete,
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
            player: 5,
            action_type: ActionType::ForeignAid,
        },
        Action {
            player: 5,
            action_type: ActionType::Complete,
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
    ]
}

#[cfg(test)]
mod tests {
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    use super::*;

    #[test]
    fn income_should_add_coin_without_blockers_and_set_next_player() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Income,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.players[0].coins, 3);
        assert_eq!(game.player, 1);
    }

    #[test]
    fn foreign_aid_should_add_counteraction_blocker() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::ForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::ForeignAid,
                target: 0,
                source: None,
            }
        ]);
    }

    #[test]
    fn block_foreign_aid_should_add_counteraction_blocker() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::ForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::BlockForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::ForeignAid,
                target: 0,
                source: Some(1),
            },
            Blocker::Counteraction {
                action_type: ActionType::BlockForeignAid,
                target: 1,
                source: None,
            },
        ]);
    }

    #[test]
    fn complete_after_block_foreign_should_remove_blockers_and_set_next_player() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::ForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::BlockForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::Complete,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![]);
        assert_eq!(game.players, vec![
            Player {
                coins: 2,
                cards: vec![
                    PlayerCard { kind: Card::Ambassador, revealed: false },
                    PlayerCard { kind: Card::Contessa, revealed: false },
                ],
            },
            Player {
                coins: 2,
                cards: vec![
                    PlayerCard { kind: Card::Captain, revealed: false },
                    PlayerCard { kind: Card::Duke, revealed: false },
                ],
            },
        ]);
        assert_eq!(game.player, 1);
    }

    #[test]
    fn challenge_after_block_foreign_should_add_challenge_blocker() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::ForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::BlockForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Challenge,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::ForeignAid,
                target: 0,
                source: Some(1),
            },
            Blocker::Counteraction {
                action_type: ActionType::BlockForeignAid,
                target: 1,
                source: Some(0),
            },
            Blocker::Challenge {
                target: 1,
                source: 0,
                card: Card::Duke,
            }
        ]);
    }

    #[test]
    fn show_card_after_challenge_should_replace_challenge_by_reveal_card_blocker() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::ForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::BlockForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Challenge,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::ShowCard(Card::Duke),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::ForeignAid,
                target: 0,
                source: Some(1),
            },
            Blocker::Counteraction {
                action_type: ActionType::BlockForeignAid,
                target: 1,
                source: Some(0),
            },
            Blocker::RevealCard {
                target: 0,
            }
        ]);
    }

    #[test]
    fn reveal_card_after_reveal_card_should_remove_blockers_and_set_next_player() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::ForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::BlockForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Challenge,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::ShowCard(Card::Duke),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::RevealCard(game.players[0].cards[0].kind),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![]);
        assert_eq!(game.players, vec![
            Player {
                coins: 2,
                cards: vec![
                    PlayerCard { kind: Card::Ambassador, revealed: true },
                    PlayerCard { kind: Card::Contessa, revealed: false },
                ],
            },
            Player {
                coins: 2,
                cards: vec![
                    PlayerCard { kind: Card::Captain, revealed: false },
                    PlayerCard { kind: Card::Assassin, revealed: false },
                ],
            },
        ]);
        assert_eq!(game.player, 1);
    }

    #[test]
    fn reveal_card_after_challenge_should_remove_blockers_and_complete_action_and_set_next_player() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::ForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::BlockForeignAid,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Challenge,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::RevealCard(game.players[1].cards[0].kind),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![]);
        assert_eq!(game.players, vec![
            Player {
                coins: 4,
                cards: vec![
                    PlayerCard { kind: Card::Ambassador, revealed: false },
                    PlayerCard { kind: Card::Contessa, revealed: false },
                ],
            },
            Player {
                coins: 2,
                cards: vec![
                    PlayerCard { kind: Card::Captain, revealed: true },
                    PlayerCard { kind: Card::Duke, revealed: false },
                ],
            },
        ]);
        assert_eq!(game.player, 1);
    }

    #[test]
    fn complete_after_tax_should_advance_game() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Tax,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Complete,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![]);
        assert_eq!(game.players[0].coins, 5);
        assert_eq!(game.player, 1);
    }

    #[test]
    fn challenge_after_tax_should_add_challenge() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Tax,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::Challenge,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::Tax,
                target: 0,
                source: Some(1),
            },
            Blocker::Challenge {
                target: 0,
                source: 1,
                card: Card::Duke,
            },
        ]);
    }

    #[test]
    fn coup_should_subtract_7_coins_and_add_reveal_card_blocker() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.players[0].coins = 7;
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Coup(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::RevealCard {
                target: 1,
            },
        ]);
        assert_eq!(game.players[0].coins, 0);
    }

    #[test]
    fn steal_should_add_counteraction() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Steal(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::Steal(1),
                target: 0,
                source: None,
            },
        ]);
    }

    #[test]
    fn block_steal_after_steal_should_add_counteraction() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Steal(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::BlockSteal(Card::Ambassador),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::Steal(1),
                target: 0,
                source: Some(1),
            },
            Blocker::Counteraction {
                action_type: ActionType::BlockSteal(Card::Ambassador),
                target: 1,
                source: None,
            },
        ]);
    }

    #[test]
    fn successful_challenged_block_steal_should_prevent_steal() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.players[1].cards[0].kind = Card::Ambassador;
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Steal(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::BlockSteal(Card::Ambassador),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Challenge,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::ShowCard(Card::Ambassador),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::RevealCard(game.players[0].cards[0].kind),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![]);
        assert_eq!(game.players[0].coins, 2);
        assert_eq!(game.players[1].coins, 2);
        assert_eq!(game.player, 1);
    }

    #[test]
    fn complete_after_steal_should_advance_game() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Steal(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Complete,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![]);
        assert_eq!(game.players[0].coins, 4);
        assert_eq!(game.players[1].coins, 0);
        assert_eq!(game.player, 1);
    }

    #[test]
    fn challenge_after_steal_should_add_challenge_blocker() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.players[1].coins = 2;
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Steal(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::Challenge,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::Steal(1),
                target: 0,
                source: Some(1),
            },
            Blocker::Challenge {
                target: 0,
                source: 1,
                card: Card::Captain,
            },
        ]);
    }

    #[test]
    fn assassinate_should_add_counteraction() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.players[0].coins = 3;
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Assassinate(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::Assassinate(1),
                target: 0,
                source: None,
            },
        ]);
        assert_eq!(game.players[0].coins, 0);
    }

    #[test]
    fn complete_after_assassinate_should_add_reveal_card_blocker() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.players[0].coins = 3;
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Assassinate(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Complete,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::RevealCard {
                target: 1,
            },
        ]);
    }

    #[test]
    fn challenge_after_assassinate_should_add_challenge_blocker() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.players[0].coins = 3;
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Assassinate(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::Challenge,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::Assassinate(1),
                target: 0,
                source: Some(1),
            },
            Blocker::Challenge {
                target: 0,
                source: 1,
                card: Card::Assassin,
            },
        ]);
    }

    #[test]
    fn block_assassinate_after_assassinate_should_add_counteraction() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.players[0].coins = 3;
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Assassinate(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::BlockAssassination,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::Assassinate(1),
                target: 0,
                source: Some(1),
            },
            Blocker::Counteraction {
                action_type: ActionType::BlockAssassination,
                target: 1,
                source: None,
            },
        ]);
    }

    #[test]
    fn complete_after_block_assassinate_should_advance_game() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.players[0].coins = 3;
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Assassinate(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::BlockAssassination,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::Complete,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![]);
        assert_eq!(game.player, 1);
    }

    #[test]
    fn challenge_after_block_assassinate_should_add_challenge() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.players[0].coins = 3;
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Assassinate(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::BlockAssassination,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Challenge,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::Assassinate(1),
                target: 0,
                source: Some(1),
            },
            Blocker::Counteraction {
                action_type: ActionType::BlockAssassination,
                target: 1,
                source: Some(0),
            },
            Blocker::Challenge {
                target: 1,
                source: 0,
                card: Card::Contessa,
            },
        ]);
    }

    #[test]
    fn reveal_card_after_challenged_after_block_assassinate_should_add_reveal_card_blocker() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        game.players[0].coins = 3;
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Assassinate(1),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::BlockAssassination,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Challenge,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::RevealCard(game.players[1].cards[0].kind),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.players, vec![
            Player {
                coins: 0,
                cards: vec![
                    PlayerCard { kind: Card::Ambassador, revealed: false },
                    PlayerCard { kind: Card::Contessa, revealed: false },
                ],
            },
            Player {
                coins: 2,
                cards: vec![
                    PlayerCard { kind: Card::Captain, revealed: true },
                    PlayerCard { kind: Card::Duke, revealed: false },
                ],
            },
        ]);
        assert_eq!(game.blockers, vec![
            Blocker::RevealCard {
                target: 1,
            },
        ]);
    }

    #[test]
    fn exchange_should_add_counteraction() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Exchange,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::Exchange,
                target: 0,
                source: None,
            },
        ]);
        assert_eq!(game.players[0].coins, 2);
    }

    #[test]
    fn complete_after_exchange_should_add_drop_cards_blocker() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Exchange,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Complete,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::DropCard {
                target: 0,
            },
        ]);
    }

    #[test]
    fn drop_cards_should_advance_game() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Exchange,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Complete,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::DropCard(game.players[0].cards[0].kind),
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.players, vec![
            Player {
                coins: 2,
                cards: vec![
                    PlayerCard { kind: Card::Contessa, revealed: false },
                    PlayerCard { kind: Card::Assassin, revealed: false },
                ],
            },
            Player {
                coins: 2,
                cards: vec![
                    PlayerCard { kind: Card::Captain, revealed: false },
                    PlayerCard { kind: Card::Duke, revealed: false },
                ],
            },
        ]);
        assert_eq!(game.blockers, vec![]);
    }

    #[test]
    fn challenge_after_exchange_should_add_challenge_blocker() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(Settings { players_number: 2, cards_per_type: 1 }, &mut rng);
        assert_eq!(
            game.play(&Action {
                player: 0,
                action_type: ActionType::Exchange,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(
            game.play(&Action {
                player: 1,
                action_type: ActionType::Challenge,
            }, &mut rng),
            Ok(())
        );
        assert_eq!(game.blockers, vec![
            Blocker::Counteraction {
                action_type: ActionType::Exchange,
                target: 0,
                source: Some(1),
            },
            Blocker::Challenge {
                target: 0,
                source: 1,
                card: Card::Ambassador,
            },
        ]);
    }

    #[test]
    fn play_full_game_should_set_a_winner() {
        let actions = get_example_actions();
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(get_example_settings(), &mut rng);
        for i in 0..actions.len() {
            let action = &actions[i];
            let view = game.get_anonymous_view();
            let allowed_actions = get_available_actions(view.player, &view.players, view.blockers);
            assert!(allowed_actions.contains(action), "action={:?} allowed={:?}", action, allowed_actions);
            assert_eq!(game.play(action, &mut rng), Ok(()), "{}) action={:?} player={:?} allowed={:?}", i, action, game.players[action.player], allowed_actions);
        }
        assert!(game.is_done());
        assert_eq!(game.get_winner(), Some(4));
        assert_eq!(game.step(), actions.len());
        assert_eq!(game.turn(), 44);
        assert_eq!(game.round(), 8);
    }
}
