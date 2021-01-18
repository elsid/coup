use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use itertools::Itertools;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use crate::game::{Action, ActionType, ALL_CARDS, Card, CARDS_PER_PLAYER, MAX_CARDS_TO_EXCHANGE, PlayerCard, PlayerView, Settings};

pub trait Bot {
    fn suggest_actions<'a>(&mut self, view: &PlayerView, available_actions: &'a Vec<Action>) -> Vec<&'a Action>;

    fn suggest_optional_actions<'a>(&mut self, view: &PlayerView, available_actions: &'a Vec<Action>) -> Vec<&'a Action>;

    fn get_action(&mut self, view: &PlayerView, available_actions: &Vec<Action>) -> Action;

    fn get_optional_action(&mut self, view: &PlayerView, available_actions: &Vec<Action>) -> Option<Action>;

    fn after_player_action(&mut self, view: &PlayerView, action: &Action);

    fn after_opponent_action(&mut self, view: &PlayerView, action: &ActionView);
}

#[derive(Debug, Clone)]
pub struct ActionView {
    player: usize,
    action_type: ActionTypeView,
}

impl ActionView {
    pub fn from_action(value: &Action) -> Self {
        Self {
            player: value.player,
            action_type: ActionTypeView::from_action_type(&value.action_type),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum ActionTypeView {
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
    DropCard,
}

impl ActionTypeView {
    fn from_action_type(value: &ActionType) -> Self {
        match value {
            ActionType::Income => ActionTypeView::Income,
            ActionType::ForeignAid => ActionTypeView::ForeignAid,
            ActionType::Coup(target) => ActionTypeView::Coup(*target),
            ActionType::Tax => ActionTypeView::Tax,
            ActionType::Assassinate(target) => ActionTypeView::Assassinate(*target),
            ActionType::Exchange => ActionTypeView::Exchange,
            ActionType::Steal(target) => ActionTypeView::Steal(*target),
            ActionType::BlockForeignAid => ActionTypeView::BlockForeignAid,
            ActionType::BlockAssassination => ActionTypeView::BlockAssassination,
            ActionType::BlockSteal(card) => ActionTypeView::BlockSteal(*card),
            ActionType::Complete => ActionTypeView::Complete,
            ActionType::Challenge => ActionTypeView::Challenge,
            ActionType::ShowCard(card) => ActionTypeView::ShowCard(*card),
            ActionType::RevealCard(card) => ActionTypeView::RevealCard(*card),
            ActionType::DropCard(..) => ActionTypeView::DropCard,
        }
    }
}

#[derive(Clone)]
pub struct RandomBot {
    rng: StdRng,
}

impl RandomBot {
    pub fn new(view: &PlayerView) -> Self {
        Self {
            rng: make_rng_from_cards(view.cards),
        }
    }
}

fn make_rng_from_cards(cards: &[PlayerCard]) -> StdRng {
    let mut hasher = DefaultHasher::new();
    cards.hash(&mut hasher);
    StdRng::seed_from_u64(hasher.finish())
}

impl Bot for RandomBot {
    fn suggest_actions<'a>(&mut self, view: &PlayerView, available_actions: &'a Vec<Action>) -> Vec<&'a Action> {
        available_actions.iter()
            .filter(|action| is_allowed_action_type(&action.action_type, view.cards))
            .collect()
    }

    fn suggest_optional_actions<'a>(&mut self, view: &PlayerView, available_actions: &'a Vec<Action>) -> Vec<&'a Action> {
        self.suggest_actions(view, available_actions)
    }

    fn get_action(&mut self, view: &PlayerView, available_actions: &Vec<Action>) -> Action {
        self.suggest_actions(view, available_actions)
            .choose(&mut self.rng)
            .map(|v| *v)
            .unwrap().clone()
    }

    fn get_optional_action(&mut self, view: &PlayerView, available_actions: &Vec<Action>) -> Option<Action> {
        if self.rng.gen::<bool>() {
            Some(self.get_action(view, available_actions))
        } else {
            None
        }
    }

    fn after_player_action(&mut self, _: &PlayerView, _: &Action) {}

    fn after_opponent_action(&mut self, _: &PlayerView, _: &ActionView) {}
}

#[derive(Debug)]
struct Diff {
    added: usize,
    removed: usize,
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct CardCollection {
    known: Vec<Card>,
    any: usize,
}

impl CardCollection {
    fn new(known: Vec<Card>, len: usize) -> Self {
        Self { any: len - known.len(), known }
    }

    fn len(&self) -> usize {
        self.known.len() + self.any
    }

    fn is_empty(&self) -> bool {
        self.known.is_empty() && self.any == 0
    }

    fn known_len(&self) -> usize {
        self.known.len()
    }

    fn get_known(&self, index: usize) -> Card {
        self.known[index]
    }

    fn has_any(&self) -> bool {
        self.any > 0
    }

    fn get_added_cards(&self, new_hand: &Vec<Card>) -> Vec<Card> {
        let mut j = 0;
        let mut added = Vec::new();
        for i in 0..new_hand.len() {
            if j < self.known.len() && new_hand[i] == self.known[j] {
                j += 1;
            } else {
                added.push(new_hand[i]);
            }
        }
        added
    }

    fn get_diff(&self, hand: &Vec<Card>) -> Option<Diff> {
        let mut added = None;
        let mut removed = None;
        let mut i = 0;
        let mut j = 0;
        while i < self.known.len() && j < hand.len() {
            if self.known[i] < hand[j] {
                removed = Some(i);
                i += 1;
            } else if hand[j] < self.known[i] {
                added = Some(j);
                j += 1;
            } else {
                i += 1;
                j += 1;
            }
        }
        if i < self.known.len() {
            removed = Some(i);
        } else if j < hand.len() {
            added = Some(j);
        }
        if let (Some(added), Some(removed)) = (added, removed) {
            Some(Diff { added, removed })
        } else {
            None
        }
    }

    fn find_card(&self, card: Card) -> Option<usize> {
        self.known.iter()
            .find_position(|v| **v == card)
            .map(|(v, _)| v)
    }

    fn contains_known(&self, card: Card) -> bool {
        self.known.contains(&card)
    }

    fn count_known(&self, card: Card) -> usize {
        self.known.iter().filter(|v| **v == card).count()
    }

    fn get_card_positions(&self, card: Card) -> Vec<usize> {
        self.known.iter()
            .enumerate()
            .filter(|(_, v)| **v == card)
            .map(|(position, _)| position)
            .collect()
    }

    fn replace_any_by_known(&mut self, card: Card) {
        self.known.push(card);
        self.any -= 1;
    }

    fn add_known(&mut self, card: Card) {
        self.known.push(card);
    }

    fn remove_known(&mut self, index: usize) {
        self.known.remove(index);
    }

    fn set_known(&mut self, index: usize, card: Card) {
        self.known[index] = card;
    }

    fn add_any(&mut self) {
        self.any += 1;
    }

    fn remove_any(&mut self) {
        self.any -= 1;
    }

    fn sort(&mut self) {
        self.known.sort();
    }

    fn remove_taken_cards(&mut self, added: &Vec<Card>) -> bool {
        let mut i = 0;
        let mut j = 0;
        let mut taken_known = Vec::new();
        while i < self.known.len() && j < added.len() {
            if added[j] < self.known[i] {
                j += 1;
            } else if self.known[i] < added[j] {
                i += 1;
            } else {
                taken_known.push(i);
                i += 1;
                j += 1;
            }
        }
        let taken_any = added.len() - taken_known.len();
        if self.any < taken_any {
            return false;
        }
        taken_known.reverse();
        for i in taken_known.iter() {
            self.known.remove(*i);
        }
        self.any -= taken_any;
        true
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct GamePlayer {
    hand: CardCollection,
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct GameState {
    players: Vec<GamePlayer>,
    revealed_cards: Vec<Card>,
    deck: CardCollection,
}

impl GameState {
    fn initial(player: usize, hand: &Vec<Card>, settings: &Settings) -> Vec<Self> {
        let mut ordered_cards = hand.clone();
        ordered_cards.sort();
        let mut unique_cards = ordered_cards.clone();
        unique_cards.dedup();
        let deck_len = settings.cards_per_type * ALL_CARDS.len() - settings.players_number * CARDS_PER_PLAYER;
        let base_game_state = Self {
            players: (0..settings.players_number)
                .map(|index| {
                    if index == player {
                        GamePlayer { hand: CardCollection::new(ordered_cards.clone(), CARDS_PER_PLAYER) }
                    } else {
                        GamePlayer { hand: CardCollection::new(Vec::new(), CARDS_PER_PLAYER) }
                    }
                })
                .collect(),
            revealed_cards: Vec::new(),
            deck: CardCollection::new(Vec::new(), deck_len),
        };
        let mut result = Vec::new();
        let targets: Vec<usize> = (0..settings.players_number).into_iter()
            .filter(|v| *v != player || *v == player && deck_len > 0)
            .collect();
        if unique_cards.len() == 1 {
            if settings.cards_per_type > 2 {
                for opponents in targets.iter().combinations_with_replacement(settings.cards_per_type - 2) {
                    let mut game_state = base_game_state.clone();
                    let mut add = true;
                    for &opponent in opponents {
                        if opponent == player {
                            if !game_state.deck.has_any() {
                                add = false;
                                break;
                            }
                            game_state.deck.replace_any_by_known(unique_cards[0]);
                        } else {
                            if !game_state.players[opponent].hand.has_any() {
                                add = false;
                                break;
                            }
                            game_state.players[opponent].hand.replace_any_by_known(unique_cards[0]);
                        }
                    }
                    if add {
                        result.push(game_state);
                    }
                }
            }
        } else if unique_cards.len() == 2 {
            if settings.cards_per_type > 1 {
                for first_opponents in targets.iter().combinations_with_replacement(settings.cards_per_type - 1) {
                    for second_opponents in targets.iter().combinations_with_replacement(settings.cards_per_type - 1) {
                        let mut game_state = base_game_state.clone();
                        let mut add = true;
                        for &&opponent in first_opponents.iter() {
                            if opponent == player {
                                if !game_state.deck.has_any() {
                                    add = false;
                                    break;
                                }
                                game_state.deck.replace_any_by_known(unique_cards[0]);
                            } else {
                                if !game_state.players[opponent].hand.has_any() {
                                    add = false;
                                    break;
                                }
                                game_state.players[opponent].hand.replace_any_by_known(unique_cards[0]);
                            }
                        }
                        if !add {
                            continue;
                        }
                        for &opponent in second_opponents {
                            if opponent == player {
                                if !game_state.deck.has_any() {
                                    add = false;
                                    break;
                                }
                                game_state.deck.replace_any_by_known(unique_cards[1]);
                            } else {
                                if !game_state.players[opponent].hand.has_any() {
                                    add = false;
                                    break;
                                }
                                game_state.players[opponent].hand.replace_any_by_known(unique_cards[1]);
                            }
                        }
                        if add {
                            result.push(game_state);
                        }
                    }
                }
            }
        } else {
            panic!("Unsupported number of unique cards: {:?}", unique_cards);
        }
        if result.is_empty() {
            result.push(base_game_state);
        }
        for game_state in result.iter_mut() {
            for player in game_state.players.iter_mut() {
                player.hand.sort();
            }
            game_state.deck.sort();
        }
        result.sort();
        result.dedup();
        result
    }

    fn is_valid(&self, cards_per_type: usize) -> bool {
        ALL_CARDS.iter()
            .all(|card| {
                self.players.iter().map(|v| v.hand.count_known(*card)).sum::<usize>()
                    + self.deck.count_known(*card) <= cards_per_type
            })
    }

    fn print(&self) {
        for player in 0..self.players.len() {
            if !self.players[player].hand.is_empty() {
                print!(" {}={:?}", player, self.players[player].hand);
            }
        }
        println!(" deck={:?} revealed={:?}", self.deck, self.revealed_cards);
    }

    fn apply_for_player(&self, action: &Action, player_hand: &Vec<Card>, result: &mut Vec<Self>) {
        match &action.action_type {
            ActionType::Complete => {
                self.complete_for_player(action.player, player_hand, result);
            }
            ActionType::ShowCard(shown_card) => {
                self.show_card_for_player(*shown_card, action.player, player_hand, result);
            }
            ActionType::RevealCard(revealed_card) => {
                self.reveal_card_for_player(*revealed_card, action.player, result);
            }
            ActionType::DropCard(dropped_card) => {
                self.drop_card_for_player(*dropped_card, action.player, result);
            }
            _ => result.push(self.clone()),
        }
    }

    fn apply_for_opponent(&self, view: &PlayerView, action: &ActionView, result: &mut Vec<Self>) {
        match &action.action_type {
            ActionTypeView::Complete => {
                self.complete_for_opponent(view.players[action.player].hand, action.player, result);
            }
            ActionTypeView::ShowCard(shown_card) => {
                self.show_card_for_opponent(*shown_card, action.player, result);
            }
            ActionTypeView::RevealCard(revealed_card) => {
                self.reveal_card_for_opponent(*revealed_card, action.player, view, result);
            }
            ActionTypeView::DropCard => {
                self.drop_card_for_opponent(action.player, result);
            }
            _ => result.push(self.clone()),
        }
    }

    fn complete_for_player(&self, player: usize, player_hand: &Vec<Card>, result: &mut Vec<Self>) {
        if player_hand.len() == self.players[player].hand.len() {
            result.push(self.clone());
            return;
        }
        let added = self.players[player].hand.get_added_cards(&player_hand);
        let mut deck = self.deck.clone();
        let mut players = self.players.clone();
        for card in added.iter() {
            players[player].hand.add_known(*card);
        }
        players[player].hand.sort();
        if !deck.remove_taken_cards(&added) {
            return;
        }
        result.push(Self {
            players,
            revealed_cards: self.revealed_cards.clone(),
            deck,
        });
    }

    fn show_card_for_player(&self, shown_card: Card, player: usize, player_hand: &Vec<Card>, result: &mut Vec<Self>) {
        if let Some(diff) = self.players[player].hand.get_diff(&player_hand) {
            let mut deck = self.deck.clone();
            let deck_some = deck.find_card(shown_card);
            if let Some(index) = deck_some {
                deck.remove_known(index);
            } else if deck.has_any() {
                deck.remove_any();
            } else {
                return;
            }
            deck.add_known(self.players[player].hand.get_known(diff.removed));
            deck.sort();
            let mut players = self.players.clone();
            players[player].hand.remove_known(diff.removed);
            players[player].hand.add_known(player_hand[diff.added]);
            players[player].hand.sort();
            result.push(Self {
                players,
                revealed_cards: self.revealed_cards.clone(),
                deck,
            });
        } else {
            result.push(self.clone());
        }
    }

    fn reveal_card_for_player(&self, revealed_card: Card, player: usize, result: &mut Vec<Self>) {
        let revealed = self.players[player].hand.find_card(revealed_card).unwrap();
        let mut players = self.players.clone();
        players[player].hand.remove_known(revealed);
        let mut revealed_cards = self.revealed_cards.clone();
        revealed_cards.push(revealed_card);
        revealed_cards.sort();
        result.push(Self {
            players,
            revealed_cards,
            deck: self.deck.clone(),
        });
    }

    fn drop_card_for_player(&self, dropped_card: Card, player: usize, result: &mut Vec<Self>) {
        let dropped = self.players[player].hand.find_card(dropped_card).unwrap();
        let mut players = self.players.clone();
        players[player].hand.remove_known(dropped);
        let mut deck = self.deck.clone();
        deck.add_known(dropped_card);
        deck.sort();
        result.push(Self {
            players,
            revealed_cards: self.revealed_cards.clone(),
            deck,
        });
    }

    fn complete_for_opponent(&self, hand_len: usize, player: usize, result: &mut Vec<Self>) {
        if hand_len == self.players[player].hand.len() {
            result.push(self.clone());
            return;
        }
        let known_len = self.deck.known_len();
        let cards_count = self.deck.len().min(MAX_CARDS_TO_EXCHANGE);
        if known_len == 0 {
            let mut players = self.players.clone();
            let mut deck = self.deck.clone();
            for _ in 0..cards_count {
                players[player].hand.add_any();
                deck.remove_any();
            }
            result.push(Self {
                players,
                revealed_cards: self.revealed_cards.clone(),
                deck,
            });
            return;
        }
        for mut cards in (0..self.deck.len()).combinations(cards_count) {
            let mut players = self.players.clone();
            let mut deck = self.deck.clone();
            for card_index in cards.iter() {
                if *card_index < known_len {
                    players[player].hand.add_known(deck.get_known(*card_index));
                } else {
                    players[player].hand.add_any();
                }
            }
            players[player].hand.sort();
            cards.sort_by_key(|v| std::usize::MAX - *v);
            for card_index in cards.iter() {
                if *card_index < known_len {
                    deck.remove_known(*card_index);
                } else {
                    deck.remove_any();
                }
            }
            result.push(Self {
                players,
                revealed_cards: self.revealed_cards.clone(),
                deck,
            });
        }
    }

    fn show_card_for_opponent(&self, shown_card: Card, player: usize, result: &mut Vec<Self>) {
        let positions = self.players[player].hand.get_card_positions(shown_card);
        if positions.len() < self.players[player].hand.len() {
            let known_len = self.deck.known_len();
            if known_len < self.deck.len() {
                result.push(self.clone());
            }
            for deck_card in 0..known_len {
                if !self.players[player].hand.has_any() {
                    continue;
                }
                let mut players = self.players.clone();
                players[player].hand.remove_any();
                let mut deck = self.deck.clone();
                players[player].hand.add_known(deck.get_known(deck_card));
                players[player].hand.sort();
                deck.remove_known(deck_card);
                deck.add_known(shown_card);
                deck.sort();
                result.push(Self {
                    players,
                    revealed_cards: self.revealed_cards.clone(),
                    deck,
                });
            }
        }
        if !positions.is_empty() {
            for position in positions {
                let mut base_players = self.players.clone();
                base_players[player].hand.remove_known(position);
                let known_len = self.deck.known_len();
                if known_len < self.deck.len() {
                    let mut players = base_players.clone();
                    let mut deck = self.deck.clone();
                    players[player].hand.add_any();
                    deck.remove_any();
                    deck.add_known(shown_card);
                    deck.sort();
                    result.push(Self {
                        players,
                        revealed_cards: self.revealed_cards.clone(),
                        deck,
                    });
                }
                for deck_card in 0..known_len {
                    let mut players = base_players.clone();
                    let mut deck = self.deck.clone();
                    players[player].hand.add_known(deck.get_known(deck_card));
                    players[player].hand.sort();
                    deck.set_known(deck_card, shown_card);
                    deck.sort();
                    result.push(Self {
                        players,
                        revealed_cards: self.revealed_cards.clone(),
                        deck,
                    });
                }
            }
        }
    }

    fn reveal_card_for_opponent(&self, revealed_card: Card, player: usize, view: &PlayerView, result: &mut Vec<Self>) {
        let mut base_players = self.players.clone();
        let mut base_deck = self.deck.clone();
        if view.cards.len() > self.players[view.player].hand.len() {
            let mut player_hand: Vec<Card> = view.cards.iter()
                .filter(|v| !v.revealed)
                .map(|v| v.kind)
                .collect();
            player_hand.sort();
            let added = self.players[view.player].hand.get_added_cards(&player_hand);
            for card in added.iter() {
                base_players[view.player].hand.add_known(*card);
            }
            base_players[view.player].hand.sort();
            base_deck.remove_taken_cards(&added);
        }
        if self.players[player].hand.has_any() {
            let mut players = base_players.clone();
            players[player].hand.remove_any();
            let mut revealed_cards = self.revealed_cards.clone();
            revealed_cards.push(revealed_card);
            revealed_cards.sort();
            result.push(Self {
                players,
                revealed_cards,
                deck: base_deck.clone(),
            });
        }
        let positions = self.players[player].hand.get_card_positions(revealed_card);
        if !positions.is_empty() {
            for position in positions {
                let mut players = base_players.clone();
                players[player].hand.remove_known(position);
                let mut revealed_cards = self.revealed_cards.clone();
                revealed_cards.push(revealed_card);
                result.push(Self {
                    players,
                    revealed_cards,
                    deck: base_deck.clone(),
                });
            }
        }
    }

    fn drop_card_for_opponent(&self, player: usize, result: &mut Vec<Self>) {
        let known_len = self.players[player].hand.known_len();
        if known_len < self.players[player].hand.len() {
            let mut players = self.players.clone();
            let mut deck = self.deck.clone();
            deck.add_any();
            players[player].hand.remove_any();
            result.push(Self {
                players,
                revealed_cards: self.revealed_cards.clone(),
                deck,
            });
        }
        for position in 0..known_len {
            let mut players = self.players.clone();
            let mut deck = self.deck.clone();
            deck.add_known(players[player].hand.get_known(position));
            deck.sort();
            players[player].hand.remove_known(position);
            result.push(Self {
                players,
                revealed_cards: self.revealed_cards.clone(),
                deck,
            });
        }
    }

    fn is_safe_action_type(&self, player: usize, action_type: &ActionType, last_action: Option<&ActionView>, cards_per_type: usize) -> bool {
        match action_type {
            ActionType::ForeignAid => {
                self.count_known(Card::Duke) == cards_per_type
                    && !self.is_card_hold_by_opponent(player, Card::Duke)
            }
            ActionType::Assassinate(..) => {
                self.count_known(Card::Duke) == cards_per_type
                    && !self.is_card_hold_by_opponent(player, Card::Contessa)
            }
            ActionType::Steal(..) => {
                self.count_known(Card::Ambassador) == cards_per_type
                    && self.is_card_hold_by_opponent(player, Card::Ambassador)
                    && self.count_known(Card::Captain) == cards_per_type
                    && self.is_card_hold_by_opponent(player, Card::Captain)
            }
            ActionType::Challenge => {
                let claimed_card = match &last_action.unwrap().action_type {
                    ActionTypeView::Tax => Card::Duke,
                    ActionTypeView::Assassinate(..) => Card::Assassin,
                    ActionTypeView::Exchange => Card::Ambassador,
                    ActionTypeView::Steal(..) => Card::Captain,
                    ActionTypeView::BlockForeignAid => Card::Duke,
                    ActionTypeView::BlockAssassination => Card::Contessa,
                    ActionTypeView::BlockSteal(card) => *card,
                    _ => return true,
                };
                !self.players[last_action.unwrap().player].hand.contains_known(claimed_card)
                    && self.count_known(claimed_card) == cards_per_type
            }
            _ => true,
        }
    }

    fn count_known(&self, card: Card) -> usize {
        self.players.iter()
            .map(|player| player.hand.count_known(card))
            .sum()
    }

    fn is_card_hold_by_opponent(&self, player: usize, card: Card) -> bool {
        self.players.iter()
            .enumerate()
            .filter(|(index, _)| *index != player)
            .any(|(_, opponent)| opponent.hand.contains_known(card))
    }
}

#[derive(Clone)]
pub struct CardsTracker {
    player: usize,
    cards_per_type: usize,
    game_states: Vec<GameState>,
    last_action: Option<ActionView>,
}

impl CardsTracker {
    pub fn new(player: usize, hand: &Vec<Card>, settings: &Settings) -> Self {
        Self {
            player,
            cards_per_type: settings.cards_per_type,
            game_states: GameState::initial(player, hand, settings),
            last_action: None,
        }
    }

    pub fn after_player_action(&mut self, view: &PlayerView, action: &Action) {
        let mut player_hand: Vec<Card> = view.cards.iter()
            .filter(|v| !v.revealed)
            .map(|v| v.kind)
            .collect();
        player_hand.sort();
        let mut new_game_states = Vec::with_capacity(self.game_states.len());
        for game_state in self.game_states.iter() {
            game_state.apply_for_player(action, &player_hand, &mut new_game_states);
        }
        new_game_states.sort();
        new_game_states.dedup();
        new_game_states.retain(|game_state| game_state.is_valid(self.cards_per_type));
        self.game_states = new_game_states;
        self.last_action = Some(ActionView::from_action(&action));
    }

    pub fn after_opponent_action(&mut self, view: &PlayerView, action: &ActionView) {
        let mut new_game_states = Vec::with_capacity(self.game_states.len());
        for game_state in self.game_states.iter() {
            game_state.apply_for_opponent(view, action, &mut new_game_states);
        }
        new_game_states.sort();
        new_game_states.dedup();
        new_game_states.retain(|game_state| game_state.is_valid(self.cards_per_type));
        self.game_states = new_game_states;
        self.last_action = Some(action.clone());
    }

    pub fn is_safe_action_type(&self, player: usize, action_type: &ActionType) -> bool {
        self.game_states.iter()
            .all(|game_state| {
                game_state.is_safe_action_type(player, action_type, self.last_action.as_ref(), self.cards_per_type)
            })
    }

    pub fn print(&self) {
        println!("player={}: {}", self.player, self.game_states.len());
        for i in 0..self.game_states.len() {
            print!("  [{}]", i);
            self.game_states[i].print();
        }
    }
}

#[derive(Clone)]
pub struct HonestCarefulRandomBot {
    cards_tracker: CardsTracker,
    rng: StdRng,
}

impl HonestCarefulRandomBot {
    pub fn new(view: &PlayerView, settings: &Settings) -> Self {
        Self {
            cards_tracker: CardsTracker::new(view.player, &view.cards.iter().map(|v| v.kind).collect(), settings),
            rng: make_rng_from_cards(view.cards),
        }
    }
}

impl Bot for HonestCarefulRandomBot {
    fn suggest_actions<'a>(&mut self, view: &PlayerView, available_actions: &'a Vec<Action>) -> Vec<&'a Action> {
        available_actions.iter()
            .filter(|action| {
                is_honest_action_type(&action.action_type, view.cards)
                    && self.cards_tracker.is_safe_action_type(view.player, &action.action_type)
            })
            .collect()
    }

    fn suggest_optional_actions<'a>(&mut self, view: &PlayerView, available_actions: &'a Vec<Action>) -> Vec<&'a Action> {
        self.suggest_actions(view, available_actions)
    }

    fn get_action(&mut self, view: &PlayerView, available_actions: &Vec<Action>) -> Action {
        self.suggest_actions(view, available_actions)
            .choose(&mut self.rng)
            .map(|v| *v)
            .unwrap().clone()
    }

    fn get_optional_action(&mut self, view: &PlayerView, available_actions: &Vec<Action>) -> Option<Action> {
        self.suggest_optional_actions(view, available_actions)
            .choose(&mut self.rng)
            .map(|v| (*v).clone())
    }

    fn after_player_action(&mut self, view: &PlayerView, action: &Action) {
        self.cards_tracker.after_player_action(view, action);
    }

    fn after_opponent_action(&mut self, view: &PlayerView, action: &ActionView) {
        self.cards_tracker.after_opponent_action(view, action);
    }
}

fn is_allowed_action_type(action_type: &ActionType, cards: &[PlayerCard]) -> bool {
    match action_type {
        ActionType::ShowCard(card) | ActionType::RevealCard(card) | ActionType::DropCard(card) => {
            cards.iter().any(|v| !v.revealed && v.kind == *card)
        }
        _ => true,
    }
}

fn is_honest_action_type(action_type: &ActionType, cards: &[PlayerCard]) -> bool {
    match action_type {
        ActionType::Tax | ActionType::BlockForeignAid => {
            cards.iter().any(|v| !v.revealed && matches!(v.kind, Card::Duke))
        }
        ActionType::Assassinate(..) => {
            cards.iter().any(|v| !v.revealed && matches!(v.kind, Card::Assassin))
        }
        ActionType::Exchange => {
            cards.iter().any(|v| !v.revealed && matches!(v.kind, Card::Ambassador))
        }
        ActionType::Steal(..) => {
            cards.iter().any(|v| !v.revealed && matches!(v.kind, Card::Captain))
        }
        ActionType::BlockAssassination => {
            cards.iter().any(|v| !v.revealed && matches!(v.kind, Card::Contessa))
        }
        ActionType::BlockSteal(card) | ActionType::ShowCard(card) | ActionType::RevealCard(card) | ActionType::DropCard(card) => {
            cards.iter().any(|v| !v.revealed && v.kind == *card)
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use crate::game::Game;

    use super::*;

    #[test]
    fn initial_game_states_for_hand_with_equal_cards_should_be_valid() {
        let settings = Settings {
            players_number: 6,
            cards_per_type: 3,
        };
        for target_player in 0..settings.players_number {
            let game_states = GameState::initial(target_player, &vec![Card::Captain, Card::Captain], &settings);
            assert_eq!(game_states.len(), 6);
            for game_state in game_states.iter() {
                assert!(game_state.is_valid(settings.cards_per_type));
                assert_eq!(game_state.revealed_cards.len(), 0);
                assert_eq!(game_state.deck.len(), 3);
                assert_eq!(game_state.players.len(), 6);
                for player in game_state.players.iter() {
                    assert_eq!(player.hand.len(), 2);
                }
                assert_eq!(game_state.players[target_player].hand.known, &[Card::Captain, Card::Captain]);
            }
        }
    }

    #[test]
    fn initial_game_states_for_hand_with_different_cards_should_be_valid() {
        let settings = Settings {
            players_number: 6,
            cards_per_type: 3,
        };
        for target_player in 0..settings.players_number {
            let game_states = GameState::initial(target_player, &vec![Card::Duke, Card::Captain], &settings);
            assert_eq!(game_states.len(), 385);
            for game_state in game_states.iter() {
                assert!(game_state.is_valid(settings.cards_per_type));
                assert_eq!(game_state.revealed_cards.len(), 0);
                assert_eq!(game_state.deck.len(), 3);
                assert_eq!(game_state.players.len(), 6);
                for player in 0..game_state.players.len() {
                    assert_eq!(game_state.players[player].hand.len(), 2);
                    for player in game_state.players.iter() {
                        assert_eq!(player.hand.len(), 2);
                    }
                    assert_eq!(game_state.players[target_player].hand.known, &[Card::Captain, Card::Duke]);
                }
            }
        }
    }

    #[test]
    fn cards_tracker_should_remove_player_card_after_reveal_card() {
        let hand = vec![Card::Assassin, Card::Assassin];
        let settings = Settings {
            players_number: 2,
            cards_per_type: 2,
        };
        let mut tracker = CardsTracker::new(0, &hand, &settings);
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(settings.clone(), &mut rng);
        play(&Action { player: 0, action_type: ActionType::Exchange }, &mut game, &mut tracker, &mut rng);
        play(&Action { player: 1, action_type: ActionType::Challenge }, &mut game, &mut tracker, &mut rng);
        play(&Action { player: 0, action_type: ActionType::RevealCard(Card::Assassin) }, &mut game, &mut tracker, &mut rng);
        assert_eq!(tracker.game_states, vec![
            GameState {
                players: vec![
                    GamePlayer { hand: CardCollection { known: vec![Card::Assassin], any: 0 } },
                    GamePlayer { hand: CardCollection { known: vec![], any: 2 } },
                ],
                revealed_cards: vec![Card::Assassin],
                deck: CardCollection { known: vec![], any: 6 },
            },
        ]);
    }

    #[test]
    fn cards_tracker_should_complete_exchange_after_successful_challenge() {
        let settings = Settings {
            players_number: 2,
            cards_per_type: 2,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::custom(
            vec![
                vec![Card::Ambassador, Card::Ambassador],
                vec![Card::Assassin, Card::Assassin],
            ],
            vec![
                Card::Captain,
                Card::Duke,
                Card::Contessa,
                Card::Duke,
                Card::Captain,
                Card::Contessa,
            ],
        );
        let hand: Vec<Card> = game.get_player_view(0).cards.iter().map(|v| v.kind).collect();
        let mut tracker = CardsTracker::new(0, &hand, &settings);
        play(&Action { player: 0, action_type: ActionType::Exchange }, &mut game, &mut tracker, &mut rng);
        play(&Action { player: 1, action_type: ActionType::Challenge }, &mut game, &mut tracker, &mut rng);
        play(&Action { player: 0, action_type: ActionType::ShowCard(Card::Ambassador) }, &mut game, &mut tracker, &mut rng);
        play(&Action { player: 1, action_type: ActionType::RevealCard(Card::Assassin) }, &mut game, &mut tracker, &mut rng);
        assert_eq!(tracker.game_states, vec![
            GameState {
                players: vec![
                    GamePlayer { hand: CardCollection { known: vec![Card::Ambassador, Card::Captain, Card::Duke, Card::Duke], any: 0 } },
                    GamePlayer { hand: CardCollection { known: vec![], any: 1 } },
                ],
                revealed_cards: vec![Card::Assassin],
                deck: CardCollection { known: vec![Card::Ambassador], any: 3 },
            },
        ]);
    }

    fn play<R: Rng>(action: &Action, game: &mut Game, tracker: &mut CardsTracker, rng: &mut R) {
        assert_eq!(game.play(action, rng), Ok(()));
        if action.player == 0 {
            tracker.after_player_action(&game.get_player_view(0), action);
        } else {
            tracker.after_opponent_action(&game.get_player_view(0), &ActionView::from_action(&action));
        }
    }
}
