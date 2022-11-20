use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use itertools::Itertools;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use crate::fsm::{
    play_action, Action, ActionType, Card, ConstRng, Deck, Error, PlayerCards, State, StateType,
    CARDS_PER_PLAYER, MAX_CARDS_TO_EXCHANGE,
};
use crate::game::{PlayerView, Settings, ALL_CARDS, INITIAL_COINS};

pub trait Bot {
    fn suggest_actions<'a>(
        &mut self,
        view: &PlayerView,
        available_actions: &'a Vec<Action>,
    ) -> Vec<&'a Action>;

    fn suggest_optional_actions<'a>(
        &mut self,
        view: &PlayerView,
        available_actions: &'a Vec<Action>,
    ) -> Vec<&'a Action>;

    fn get_action(&mut self, view: &PlayerView, available_actions: &Vec<Action>) -> Action;

    fn get_optional_action(
        &mut self,
        view: &PlayerView,
        available_actions: &Vec<Action>,
    ) -> Option<Action>;

    fn after_player_action(&mut self, view: &PlayerView, action: &Action);

    fn after_opponent_action(&mut self, view: &PlayerView, action: &ActionView);

    fn query(&self, command: &str);
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
    PassChallenge,
    PassBlock,
    Challenge,
    ShowCard(Card),
    RevealCard(Card),
    DropCard,
    TakeCard,
    ShuffleDeck,
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
            ActionType::PassChallenge => ActionTypeView::PassChallenge,
            ActionType::PassBlock => ActionTypeView::PassBlock,
            ActionType::Challenge => ActionTypeView::Challenge,
            ActionType::ShowCard(card) => ActionTypeView::ShowCard(*card),
            ActionType::RevealCard(card) => ActionTypeView::RevealCard(*card),
            ActionType::DropCard(..) => ActionTypeView::DropCard,
            ActionType::TakeCard => ActionTypeView::TakeCard,
            ActionType::ShuffleDeck => ActionTypeView::ShuffleDeck,
        }
    }

    fn as_action_type(&self) -> ActionType {
        match self {
            ActionTypeView::Income => ActionType::Income,
            ActionTypeView::ForeignAid => ActionType::ForeignAid,
            ActionTypeView::Coup(target) => ActionType::Coup(*target),
            ActionTypeView::Tax => ActionType::Tax,
            ActionTypeView::Assassinate(target) => ActionType::Assassinate(*target),
            ActionTypeView::Exchange => ActionType::Exchange,
            ActionTypeView::Steal(target) => ActionType::Steal(*target),
            ActionTypeView::BlockForeignAid => ActionType::BlockForeignAid,
            ActionTypeView::BlockAssassination => ActionType::BlockAssassination,
            ActionTypeView::BlockSteal(card) => ActionType::BlockSteal(*card),
            ActionTypeView::PassChallenge => ActionType::PassChallenge,
            ActionTypeView::PassBlock => ActionType::PassBlock,
            ActionTypeView::Challenge => ActionType::Challenge,
            ActionTypeView::ShowCard(card) => ActionType::ShowCard(*card),
            ActionTypeView::RevealCard(card) => ActionType::RevealCard(*card),
            ActionTypeView::TakeCard => ActionType::TakeCard,
            ActionTypeView::ShuffleDeck => ActionType::ShuffleDeck,
            v => panic!("No conversion to ActionType for {:?}", v),
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

fn make_rng_from_cards(cards: &[Card]) -> StdRng {
    let mut hasher = DefaultHasher::new();
    cards.hash(&mut hasher);
    StdRng::seed_from_u64(hasher.finish())
}

impl Bot for RandomBot {
    fn suggest_actions<'a>(
        &mut self,
        view: &PlayerView,
        available_actions: &'a Vec<Action>,
    ) -> Vec<&'a Action> {
        available_actions
            .iter()
            .filter(|action| is_allowed_action_type(&action.action_type, view.cards))
            .collect()
    }

    fn suggest_optional_actions<'a>(
        &mut self,
        view: &PlayerView,
        available_actions: &'a Vec<Action>,
    ) -> Vec<&'a Action> {
        self.suggest_actions(view, available_actions)
    }

    fn get_action(&mut self, view: &PlayerView, available_actions: &Vec<Action>) -> Action {
        self.suggest_actions(view, available_actions)
            .choose(&mut self.rng)
            .map(|v| *v)
            .unwrap()
            .clone()
    }

    fn get_optional_action(
        &mut self,
        view: &PlayerView,
        available_actions: &Vec<Action>,
    ) -> Option<Action> {
        if self.rng.gen::<bool>() {
            Some(self.get_action(view, available_actions))
        } else {
            None
        }
    }

    fn after_player_action(&mut self, _: &PlayerView, _: &Action) {}

    fn after_opponent_action(&mut self, _: &PlayerView, _: &ActionView) {}

    fn query(&self, _: &str) {}
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct CardCollection {
    known: Vec<Card>,
    unknown: usize,
}

impl CardCollection {
    fn len(&self) -> usize {
        self.known.len() + self.unknown
    }

    fn is_empty(&self) -> bool {
        self.known.is_empty() && self.unknown == 0
    }

    fn has_any(&self) -> bool {
        self.unknown > 0
    }

    fn contains_known(&self, card: Card) -> bool {
        self.known.contains(&card)
    }

    fn count_known(&self, card: Card) -> usize {
        self.known.iter().filter(|v| **v == card).count()
    }

    fn replace_any_by_known(&mut self, card: Card) {
        self.known.push(card);
        self.unknown -= 1;
    }

    fn sort(&mut self) {
        self.known.sort();
    }
}

impl PlayerCards for CardCollection {
    fn has_card(&self, card: Card) -> bool {
        self.unknown > 0 || self.known.contains(&card)
    }

    fn count(&self) -> usize {
        self.unknown + self.known.len()
    }

    fn add_card(&mut self, card: Card) {
        if matches!(card, Card::Unknown) {
            self.unknown += 1;
        } else {
            self.known.push(card);
            self.known.sort();
        }
    }

    fn drop_card(&mut self, card: Card) {
        if matches!(card, Card::Unknown) {
            self.unknown -= 1;
        } else {
            let index = self
                .known
                .iter()
                .find_position(|v| **v == card)
                .map(|(index, _)| index);
            if let Some(index) = index {
                self.known.remove(index);
            } else {
                self.unknown -= 1;
            }
        }
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum GamePlayerCards {
    Player(Vec<Card>),
    Opponent(CardCollection),
}

impl GamePlayerCards {
    fn is_empty(&self) -> bool {
        match self {
            GamePlayerCards::Player(cards) => cards.is_empty(),
            GamePlayerCards::Opponent(cards) => cards.is_empty(),
        }
    }

    fn has_any(&self) -> bool {
        match self {
            GamePlayerCards::Player(..) => unimplemented!(),
            GamePlayerCards::Opponent(cards) => cards.has_any(),
        }
    }

    fn known_len(&self) -> usize {
        match self {
            GamePlayerCards::Player(cards) => cards.len(),
            GamePlayerCards::Opponent(cards) => cards.known.len(),
        }
    }

    fn get_known(&self, index: usize) -> Card {
        match self {
            GamePlayerCards::Player(cards) => cards[index],
            GamePlayerCards::Opponent(cards) => cards.known[index],
        }
    }

    fn contains_known(&self, card: Card) -> bool {
        match self {
            GamePlayerCards::Player(cards) => cards.contains(&card),
            GamePlayerCards::Opponent(cards) => cards.contains_known(card),
        }
    }

    fn count_known(&self, card: Card) -> usize {
        match self {
            GamePlayerCards::Player(cards) => cards.iter().filter(|v| **v == card).count(),
            GamePlayerCards::Opponent(cards) => cards.count_known(card),
        }
    }

    fn replace_any_by_known(&mut self, card: Card) {
        match self {
            GamePlayerCards::Player(..) => unimplemented!(),
            GamePlayerCards::Opponent(cards) => cards.replace_any_by_known(card),
        }
    }

    fn sort(&mut self) {
        match self {
            GamePlayerCards::Player(cards) => cards.sort(),
            GamePlayerCards::Opponent(cards) => cards.sort(),
        }
    }
}

impl PlayerCards for GamePlayerCards {
    fn has_card(&self, card: Card) -> bool {
        match self {
            GamePlayerCards::Player(cards) => cards.contains(&card),
            GamePlayerCards::Opponent(cards) => cards.has_card(card),
        }
    }

    fn count(&self) -> usize {
        match self {
            GamePlayerCards::Player(cards) => cards.len(),
            GamePlayerCards::Opponent(cards) => cards.unknown + cards.known.len(),
        }
    }

    fn add_card(&mut self, card: Card) {
        match self {
            GamePlayerCards::Player(cards) => {
                cards.push(card);
                cards.sort();
            }
            GamePlayerCards::Opponent(cards) => cards.add_card(card),
        }
    }

    fn drop_card(&mut self, card: Card) {
        match self {
            GamePlayerCards::Player(cards) => cards.drop_card(card),
            GamePlayerCards::Opponent(cards) => cards.drop_card(card),
        }
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
struct GameState {
    valid: bool,
    state_type: StateType,
    player_coins: Vec<usize>,
    player_hands: Vec<usize>,
    player_cards_counter: Vec<usize>,
    player_cards: Vec<GamePlayerCards>,
    revealed_cards: Vec<Card>,
    deck: CardCollection,
}

impl GameState {
    fn initial(player: usize, cards: &Vec<Card>, settings: &Settings) -> Vec<Self> {
        let mut ordered_cards = cards.clone();
        ordered_cards.sort();
        let mut unique_cards = ordered_cards.clone();
        unique_cards.dedup();
        let deck_len =
            settings.cards_per_type * ALL_CARDS.len() - settings.players_number * CARDS_PER_PLAYER;
        let base_game_state = Self {
            valid: true,
            state_type: StateType::Turn { player: 0 },
            player_coins: std::iter::repeat(INITIAL_COINS)
                .take(settings.players_number)
                .collect(),
            player_hands: std::iter::repeat(CARDS_PER_PLAYER)
                .take(settings.players_number)
                .collect(),
            player_cards_counter: std::iter::repeat(CARDS_PER_PLAYER)
                .take(settings.players_number)
                .collect(),
            player_cards: (0..settings.players_number)
                .map(|index| {
                    if index == player {
                        GamePlayerCards::Player(cards.clone())
                    } else {
                        GamePlayerCards::Opponent(CardCollection {
                            known: Vec::with_capacity(CARDS_PER_PLAYER + MAX_CARDS_TO_EXCHANGE),
                            unknown: CARDS_PER_PLAYER,
                        })
                    }
                })
                .collect(),
            revealed_cards: Vec::with_capacity(settings.cards_per_type * ALL_CARDS.len()),
            deck: CardCollection {
                known: Vec::with_capacity(CARDS_PER_PLAYER + MAX_CARDS_TO_EXCHANGE),
                unknown: deck_len,
            },
        };
        let mut result = Vec::new();
        let targets: Vec<usize> = (0..settings.players_number)
            .into_iter()
            .filter(|v| *v != player || *v == player && deck_len > 0)
            .collect();
        if unique_cards.len() == 1 {
            if settings.cards_per_type > 2 {
                for opponents in targets
                    .iter()
                    .combinations_with_replacement(settings.cards_per_type - 2)
                {
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
                            if !game_state.player_cards[opponent].has_any() {
                                add = false;
                                break;
                            }
                            game_state.player_cards[opponent].replace_any_by_known(unique_cards[0]);
                        }
                    }
                    if add {
                        result.push(game_state);
                    }
                }
            }
        } else if unique_cards.len() == 2 {
            if settings.cards_per_type > 1 {
                for first_opponents in targets
                    .iter()
                    .combinations_with_replacement(settings.cards_per_type - 1)
                {
                    for second_opponents in targets
                        .iter()
                        .combinations_with_replacement(settings.cards_per_type - 1)
                    {
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
                                if !game_state.player_cards[opponent].has_any() {
                                    add = false;
                                    break;
                                }
                                game_state.player_cards[opponent]
                                    .replace_any_by_known(unique_cards[0]);
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
                                if !game_state.player_cards[opponent].has_any() {
                                    add = false;
                                    break;
                                }
                                game_state.player_cards[opponent]
                                    .replace_any_by_known(unique_cards[1]);
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
            for player in game_state.player_cards.iter_mut() {
                player.sort();
            }
            game_state.deck.sort();
        }
        result.sort();
        result.dedup();
        result
    }

    fn print(&self) {
        for player in 0..self.player_cards.len() {
            if !self.player_cards[player].is_empty() {
                match &self.player_cards[player] {
                    GamePlayerCards::Player(cards) => {
                        print!(" {}={:?}", player, cards);
                    }
                    GamePlayerCards::Opponent(cards) => {
                        print!(" {}={{u: {}, k: {:?}}}", player, cards.unknown, cards.known);
                    }
                }
            }
        }
        println!(
            " deck={{u: {}, k: {:?}}} revealed={:?}",
            self.deck.unknown, self.deck.known, self.revealed_cards
        );
    }

    fn is_safe_action_type(
        &self,
        player: usize,
        action_type: &ActionType,
        last_action: Option<&ActionView>,
        cards_per_type: usize,
    ) -> bool {
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
                !self.player_cards[last_action.unwrap().player].contains_known(claimed_card)
                    && self.count_known(claimed_card) == cards_per_type
            }
            _ => true,
        }
    }

    fn count_known(&self, card: Card) -> usize {
        self.player_cards
            .iter()
            .map(|player| player.count_known(card))
            .sum()
    }

    fn is_card_hold_by_opponent(&self, player: usize, card: Card) -> bool {
        self.player_cards
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != player)
            .any(|(_, opponent)| opponent.contains_known(card))
    }

    fn with_default<F: FnMut(&mut State<GamePlayerCards, CardCollection>) -> Result<(), Error>>(
        &mut self,
        mut f: F,
    ) {
        let result = f(&mut State {
            state_type: &mut self.state_type,
            player_coins: &mut self.player_coins,
            player_hands: &mut self.player_hands,
            player_cards_counter: &mut self.player_cards_counter,
            player_cards: &mut self.player_cards,
            deck: &mut self.deck,
            revealed_cards: &mut self.revealed_cards,
        });
        self.valid = matches!(result, Ok(..));
    }

    fn with_pop_known_from_deck<
        F: FnMut(&mut State<GamePlayerCards, PopKnownFromDeck>) -> Result<(), Error>,
    >(
        &mut self,
        card: Card,
        mut f: F,
    ) {
        let result = f(&mut State {
            state_type: &mut self.state_type,
            player_coins: &mut self.player_coins,
            player_hands: &mut self.player_hands,
            player_cards_counter: &mut self.player_cards_counter,
            player_cards: &mut self.player_cards,
            deck: &mut PopKnownFromDeck {
                deck: &mut self.deck,
                card,
            },
            revealed_cards: &mut self.revealed_cards,
        });
        self.valid = matches!(result, Ok(..));
    }

    fn with_pop_unknown_from_deck<
        F: FnMut(&mut State<GamePlayerCards, PopUnknownFromDeck>) -> Result<(), Error>,
    >(
        &mut self,
        mut f: F,
    ) {
        let result = f(&mut State {
            state_type: &mut self.state_type,
            player_coins: &mut self.player_coins,
            player_hands: &mut self.player_hands,
            player_cards_counter: &mut self.player_cards_counter,
            player_cards: &mut self.player_cards,
            deck: &mut PopUnknownFromDeck {
                deck: &mut self.deck,
            },
            revealed_cards: &mut self.revealed_cards,
        });
        self.valid = matches!(result, Ok(..));
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
        for game_state in self.game_states.iter_mut() {
            if game_state.deck.len() > view.deck {
                let card = if let GamePlayerCards::Player(cards) =
                    &game_state.player_cards[action.player]
                {
                    view.cards
                        .iter()
                        .zip(cards.iter())
                        .find(|(l, r)| **l != **r)
                        .map(|(view_card, _)| *view_card)
                        .unwrap_or_else(|| view.cards.last().unwrap().clone())
                } else {
                    panic!(
                        "Player has invalid kind of cards: {:?}",
                        game_state.player_cards[action.player]
                    );
                };
                if !game_state.deck.has_any() && !game_state.deck.contains_known(card) {
                    game_state.valid = false;
                    continue;
                }
                game_state.with_pop_known_from_deck(card, |state| {
                    play_action(action, state, &mut ConstRng)
                });
                continue;
            }
            game_state.with_default(|state| play_action(action, state, &mut ConstRng));
        }
        self.game_states.sort();
        self.game_states.dedup();
        self.game_states.retain(|game_state| game_state.valid);
        self.last_action = Some(ActionView::from_action(&action));
    }

    pub fn after_opponent_action(&mut self, view: &PlayerView, action_view: &ActionView) {
        for i in 0..self.game_states.len() {
            if self.game_states[i].player_cards_counter[action_view.player]
                == view.player_cards[action_view.player]
            {
                let action_type = action_view.action_type.as_action_type();
                let action = Action {
                    player: action_view.player,
                    action_type,
                };
                let game_state = &mut self.game_states[i];
                game_state.with_default(|state| play_action(&action, state, &mut ConstRng));
                continue;
            }
            if self.game_states[i].revealed_cards.len() != view.revealed_cards.len() {
                let action_type = match action_view.action_type {
                    ActionTypeView::RevealCard(card) => ActionType::RevealCard(card),
                    _ => unimplemented!(),
                };
                let action = Action {
                    player: action_view.player,
                    action_type,
                };
                let game_state = &mut self.game_states[i];
                game_state.with_default(|state| play_action(&action, state, &mut ConstRng));
                continue;
            }
            if self.game_states[i].deck.len() < view.deck {
                for card in 0..self.game_states[i].player_cards[action_view.player].known_len() {
                    let action_type = match &action_view.action_type {
                        ActionTypeView::DropCard => ActionType::DropCard(
                            self.game_states[i].player_cards[action_view.player].get_known(card),
                        ),
                        ActionTypeView::ShowCard(card) => ActionType::ShowCard(*card),
                        v => panic!("No conversion to ActionType for {:?}", v),
                    };
                    let action = Action {
                        player: action_view.player,
                        action_type,
                    };
                    let mut game_state = self.game_states[i].clone();
                    game_state.with_default(|state| play_action(&action, state, &mut ConstRng));
                    if game_state.valid {
                        self.game_states.push(game_state);
                    }
                }
                if self.game_states[i].player_cards[action_view.player].has_any() {
                    let action_type = match &action_view.action_type {
                        ActionTypeView::DropCard => ActionType::DropCard(Card::Unknown),
                        ActionTypeView::ShowCard(card) => ActionType::ShowCard(*card),
                        v => panic!("No conversion to ActionType for {:?}", v),
                    };
                    let action = Action {
                        player: action_view.player,
                        action_type,
                    };
                    let game_state = &mut self.game_states[i];
                    game_state.with_default(|state| play_action(&action, state, &mut ConstRng));
                } else {
                    self.game_states[i].valid = false;
                }
                continue;
            }
            if self.game_states[i].deck.len() > view.deck {
                for card in 0..self.game_states[i].deck.known.len() {
                    let action_type = match action_view.action_type {
                        ActionTypeView::TakeCard => ActionType::TakeCard,
                        _ => unimplemented!(),
                    };
                    let action = Action {
                        player: action_view.player,
                        action_type,
                    };
                    let mut game_state = self.game_states[i].clone();
                    game_state.with_pop_known_from_deck(game_state.deck.known[card], |state| {
                        play_action(&action, state, &mut ConstRng)
                    });
                    if game_state.valid {
                        self.game_states.push(game_state);
                    }
                }
                if self.game_states[i].deck.has_any() {
                    let action_type = match action_view.action_type {
                        ActionTypeView::TakeCard => ActionType::TakeCard,
                        _ => unimplemented!(),
                    };
                    let action = Action {
                        player: action_view.player,
                        action_type,
                    };
                    let game_state = &mut self.game_states[i];
                    game_state.with_pop_unknown_from_deck(|state| {
                        play_action(&action, state, &mut ConstRng)
                    });
                } else {
                    self.game_states[i].valid = false;
                }
                continue;
            }
            panic!("Unrecognized game state change");
        }
        self.game_states.sort();
        self.game_states.dedup();
        self.game_states.retain(|game_state| game_state.valid);
        self.last_action = Some(action_view.clone());
    }

    pub fn is_safe_action_type(&self, player: usize, action_type: &ActionType) -> bool {
        self.game_states.iter().all(|game_state| {
            game_state.is_safe_action_type(
                player,
                action_type,
                self.last_action.as_ref(),
                self.cards_per_type,
            )
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
            cards_tracker: CardsTracker::new(view.player, &view.cards.into(), settings),
            rng: make_rng_from_cards(view.cards),
        }
    }
}

impl Bot for HonestCarefulRandomBot {
    fn suggest_actions<'a>(
        &mut self,
        view: &PlayerView,
        available_actions: &'a Vec<Action>,
    ) -> Vec<&'a Action> {
        available_actions
            .iter()
            .filter(|action| {
                is_honest_action_type(&action.action_type, view.cards)
                    && self
                        .cards_tracker
                        .is_safe_action_type(view.player, &action.action_type)
            })
            .collect()
    }

    fn suggest_optional_actions<'a>(
        &mut self,
        view: &PlayerView,
        available_actions: &'a Vec<Action>,
    ) -> Vec<&'a Action> {
        self.suggest_actions(view, available_actions)
    }

    fn get_action(&mut self, view: &PlayerView, available_actions: &Vec<Action>) -> Action {
        self.suggest_actions(view, available_actions)
            .choose(&mut self.rng)
            .map(|v| *v)
            .unwrap()
            .clone()
    }

    fn get_optional_action(
        &mut self,
        view: &PlayerView,
        available_actions: &Vec<Action>,
    ) -> Option<Action> {
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

    fn query(&self, _: &str) {
        self.cards_tracker.print();
    }
}

pub fn is_allowed_action_type(action_type: &ActionType, cards: &[Card]) -> bool {
    match action_type {
        ActionType::ShowCard(card) | ActionType::RevealCard(card) | ActionType::DropCard(card) => {
            cards.iter().any(|v| *v == *card)
        }
        _ => true,
    }
}

fn is_honest_action_type(action_type: &ActionType, cards: &[Card]) -> bool {
    match action_type {
        ActionType::Tax | ActionType::BlockForeignAid => cards.contains(&Card::Duke),
        ActionType::Assassinate(..) => cards.contains(&Card::Assassin),
        ActionType::Exchange => cards.contains(&Card::Ambassador),
        ActionType::Steal(..) => cards.contains(&Card::Captain),
        ActionType::BlockAssassination => cards.contains(&Card::Contessa),
        ActionType::BlockSteal(card)
        | ActionType::ShowCard(card)
        | ActionType::RevealCard(card)
        | ActionType::DropCard(card) => cards.contains(card),
        _ => true,
    }
}

impl Deck for CardCollection {
    fn count(&self) -> usize {
        self.unknown + self.known.len()
    }

    fn pop_card(&mut self) -> Card {
        unimplemented!()
    }

    fn push_card(&mut self, card: Card) {
        if matches!(card, Card::Unknown) {
            self.unknown += 1;
        } else {
            self.known.push(card);
        }
    }

    fn shuffle<R: Rng>(&mut self, _: &mut R) {}
}

struct PopKnownFromDeck<'a> {
    deck: &'a mut CardCollection,
    card: Card,
}

impl<'a> Deck for PopKnownFromDeck<'a> {
    fn count(&self) -> usize {
        self.deck.len()
    }

    fn pop_card(&mut self) -> Card {
        self.deck.drop_card(self.card);
        self.card
    }

    fn push_card(&mut self, _: Card) {
        unimplemented!()
    }

    fn shuffle<R: Rng>(&mut self, _: &mut R) {
        unimplemented!()
    }
}

struct PopUnknownFromDeck<'a> {
    deck: &'a mut CardCollection,
}

impl<'a> Deck for PopUnknownFromDeck<'a> {
    fn count(&self) -> usize {
        self.deck.len()
    }

    fn pop_card(&mut self) -> Card {
        self.deck.unknown -= 1;
        Card::Unknown
    }

    fn push_card(&mut self, _: Card) {
        unimplemented!()
    }

    fn shuffle<R: Rng>(&mut self, _: &mut R) {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::fsm::ChallengeState;
    use crate::game::Game;

    use super::*;

    #[test]
    fn initial_game_states_for_hand_with_equal_cards_should_be_valid() {
        let settings = Settings {
            players_number: 6,
            cards_per_type: 3,
        };
        for target_player in 0..settings.players_number {
            let game_states = GameState::initial(
                target_player,
                &vec![Card::Captain, Card::Captain],
                &settings,
            );
            assert_eq!(game_states.len(), 6);
            for game_state in game_states.iter() {
                assert!(game_state.valid);
                assert_eq!(game_state.revealed_cards.len(), 0);
                assert_eq!(game_state.deck.known.len() + game_state.deck.unknown, 3);
                assert_eq!(game_state.player_coins.len(), 6);
                assert_eq!(game_state.player_hands.len(), 6);
                assert_eq!(game_state.player_cards_counter.len(), 6);
                assert_eq!(game_state.player_cards.len(), 6);
                for player in 0..game_state.player_cards.len() {
                    assert_eq!(game_state.player_coins[player], 2, "{}", player);
                    assert_eq!(game_state.player_hands[player], 2, "{}", player);
                    assert_eq!(game_state.player_cards_counter[player], 2, "{}", player);
                    assert_eq!(game_state.player_cards[player].count(), 2, "{}", player);
                    if player != target_player {
                        assert!(
                            matches!(
                                game_state.player_cards[player],
                                GamePlayerCards::Opponent(..)
                            ),
                            "{:?}",
                            game_state.player_cards[player]
                        );
                    }
                }
                assert_eq!(
                    game_state.player_cards[target_player],
                    GamePlayerCards::Player(vec![Card::Captain, Card::Captain])
                );
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
            let game_states =
                GameState::initial(target_player, &vec![Card::Duke, Card::Captain], &settings);
            assert_eq!(game_states.len(), 385);
            for game_state in game_states.iter() {
                assert!(game_state.valid);
                assert_eq!(game_state.revealed_cards.len(), 0);
                assert_eq!(game_state.deck.known.len() + game_state.deck.unknown, 3);
                assert_eq!(game_state.player_coins.len(), 6);
                assert_eq!(game_state.player_hands.len(), 6);
                assert_eq!(game_state.player_cards_counter.len(), 6);
                assert_eq!(game_state.player_cards.len(), 6);
                for player in 0..game_state.player_cards.len() {
                    assert_eq!(game_state.player_coins[player], 2, "{}", player);
                    assert_eq!(game_state.player_hands[player], 2, "{}", player);
                    assert_eq!(game_state.player_cards_counter[player], 2, "{}", player);
                    assert_eq!(game_state.player_cards[player].count(), 2, "{}", player);
                    if player != target_player {
                        assert!(
                            matches!(
                                game_state.player_cards[player],
                                GamePlayerCards::Opponent(..)
                            ),
                            "{:?}",
                            game_state.player_cards[player]
                        );
                    }
                }
                assert_eq!(
                    game_state.player_cards[target_player],
                    GamePlayerCards::Player(vec![Card::Captain, Card::Duke])
                );
            }
        }
    }

    #[test]
    fn cards_tracker_should_reveal_player_card() {
        let hand = vec![Card::Assassin, Card::Assassin];
        let settings = Settings {
            players_number: 2,
            cards_per_type: 2,
        };
        let mut tracker = CardsTracker::new(0, &hand, &settings);
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::new(settings.clone(), &mut rng);
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
                action_type: ActionType::RevealCard(Card::Assassin),
            },
        ];
        assert_eq!(
            play_actions(&actions, &mut game, &mut tracker, &mut rng),
            Ok(())
        );
        assert_eq!(
            tracker.game_states,
            vec![GameState {
                valid: true,
                state_type: StateType::Turn { player: 1 },
                player_coins: vec![2, 2],
                player_hands: vec![1, 2],
                player_cards_counter: vec![1, 2],
                player_cards: vec![
                    GamePlayerCards::Player(vec![Card::Assassin]),
                    GamePlayerCards::Opponent(CardCollection {
                        known: vec![],
                        unknown: 2
                    }),
                ],
                revealed_cards: vec![Card::Assassin],
                deck: CardCollection {
                    known: vec![],
                    unknown: 6
                },
            },]
        );
    }

    #[test]
    fn cards_tracker_should_reveal_opponent_cards() {
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
        let hand: Vec<Card> = game.get_player_view(0).cards.into();
        let mut tracker = CardsTracker::new(0, &hand, &settings);
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
                action_type: ActionType::ShowCard(Card::Ambassador),
            },
            Action {
                player: 1,
                action_type: ActionType::RevealCard(Card::Assassin),
            },
        ];
        assert_eq!(
            play_actions(&actions, &mut game, &mut tracker, &mut rng),
            Ok(())
        );
        assert_eq!(
            tracker.game_states,
            vec![GameState {
                valid: true,
                state_type: StateType::Challenge {
                    current_player: 0,
                    source: Rc::new(StateType::Exchange { player: 0 }),
                    state: ChallengeState::InitiatorRevealedCard { target: 0 },
                },
                player_coins: vec![2, 2],
                player_hands: vec![2, 1],
                player_cards_counter: vec![1, 1],
                player_cards: vec![
                    GamePlayerCards::Player(vec![Card::Ambassador]),
                    GamePlayerCards::Opponent(CardCollection {
                        known: vec![],
                        unknown: 1
                    }),
                ],
                revealed_cards: vec![Card::Assassin],
                deck: CardCollection {
                    known: vec![Card::Ambassador],
                    unknown: 6
                },
            },]
        );
    }

    #[test]
    fn cards_tracker_should_pop_cards_from_deck_for_player() {
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
        let hand: Vec<Card> = game.get_player_view(0).cards.into();
        let mut tracker = CardsTracker::new(0, &hand, &settings);
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
                action_type: ActionType::ShowCard(Card::Ambassador),
            },
            Action {
                player: 1,
                action_type: ActionType::RevealCard(Card::Assassin),
            },
            Action {
                player: 0,
                action_type: ActionType::ShuffleDeck,
            },
            Action {
                player: 0,
                action_type: ActionType::TakeCard,
            },
        ];
        assert_eq!(
            play_actions(&actions, &mut game, &mut tracker, &mut rng),
            Ok(())
        );
        assert_eq!(
            tracker.game_states,
            vec![GameState {
                valid: true,
                state_type: StateType::NeedCards {
                    player: 0,
                    count: 2
                },
                player_coins: vec![2, 2],
                player_hands: vec![2, 1],
                player_cards_counter: vec![2, 1],
                player_cards: vec![
                    GamePlayerCards::Player(vec![Card::Ambassador, Card::Duke]),
                    GamePlayerCards::Opponent(CardCollection {
                        known: vec![],
                        unknown: 1
                    }),
                ],
                revealed_cards: vec![Card::Assassin],
                deck: CardCollection {
                    known: vec![Card::Ambassador],
                    unknown: 5
                },
            },]
        );
    }

    #[test]
    fn cards_tracker_should_push_cards_to_deck_for_player() {
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
        let hand: Vec<Card> = game.get_player_view(0).cards.into();
        let mut tracker = CardsTracker::new(0, &hand, &settings);
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
                action_type: ActionType::ShowCard(Card::Ambassador),
            },
            Action {
                player: 1,
                action_type: ActionType::RevealCard(Card::Assassin),
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
                action_type: ActionType::TakeCard,
            },
            Action {
                player: 0,
                action_type: ActionType::TakeCard,
            },
            Action {
                player: 0,
                action_type: ActionType::DropCard(Card::Ambassador),
            },
            Action {
                player: 0,
                action_type: ActionType::DropCard(Card::Duke),
            },
        ];
        assert_eq!(
            play_actions(&actions, &mut game, &mut tracker, &mut rng),
            Ok(())
        );
        assert_eq!(
            tracker.game_states,
            vec![GameState {
                valid: true,
                state_type: StateType::Turn { player: 1 },
                player_coins: vec![2, 2],
                player_hands: vec![2, 1],
                player_cards_counter: vec![2, 1],
                player_cards: vec![
                    GamePlayerCards::Player(vec![Card::Captain, Card::Duke]),
                    GamePlayerCards::Opponent(CardCollection {
                        known: vec![],
                        unknown: 1
                    }),
                ],
                revealed_cards: vec![Card::Assassin],
                deck: CardCollection {
                    known: vec![Card::Ambassador, Card::Ambassador, Card::Duke],
                    unknown: 3
                },
            },]
        );
    }

    #[test]
    fn cards_tracker_should_pop_cards_from_deck_for_opponent() {
        let settings = Settings {
            players_number: 2,
            cards_per_type: 2,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::custom(
            vec![
                vec![Card::Assassin, Card::Assassin],
                vec![Card::Ambassador, Card::Ambassador],
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
        let hand: Vec<Card> = game.get_player_view(0).cards.into();
        let mut tracker = CardsTracker::new(0, &hand, &settings);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Income,
            },
            Action {
                player: 1,
                action_type: ActionType::Exchange,
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
        ];
        assert_eq!(
            play_actions(&actions, &mut game, &mut tracker, &mut rng),
            Ok(())
        );
        for game_state in tracker.game_states.iter() {
            assert!(
                matches!(
                    game_state.state_type,
                    StateType::NeedCards {
                        player: 1,
                        count: 2
                    }
                ),
                "{:?}",
                game_state.state_type
            );
        }
        assert_eq!(
            tracker.game_states,
            vec![
                GameState {
                    valid: true,
                    state_type: StateType::NeedCards {
                        player: 1,
                        count: 2
                    },
                    player_coins: vec![3, 2],
                    player_hands: vec![1, 2],
                    player_cards_counter: vec![1, 2],
                    player_cards: vec![
                        GamePlayerCards::Player(vec![Card::Assassin]),
                        GamePlayerCards::Opponent(CardCollection {
                            known: vec![],
                            unknown: 2
                        }),
                    ],
                    revealed_cards: vec![Card::Assassin],
                    deck: CardCollection {
                        known: vec![Card::Ambassador],
                        unknown: 5
                    },
                },
                GameState {
                    valid: true,
                    state_type: StateType::NeedCards {
                        player: 1,
                        count: 2
                    },
                    player_coins: vec![3, 2],
                    player_hands: vec![1, 2],
                    player_cards_counter: vec![1, 2],
                    player_cards: vec![
                        GamePlayerCards::Player(vec![Card::Assassin]),
                        GamePlayerCards::Opponent(CardCollection {
                            known: vec![Card::Ambassador],
                            unknown: 1
                        }),
                    ],
                    revealed_cards: vec![Card::Assassin],
                    deck: CardCollection {
                        known: vec![],
                        unknown: 6
                    },
                },
            ]
        );
    }

    #[test]
    fn cards_tracker_should_push_cards_to_deck_for_opponent() {
        let settings = Settings {
            players_number: 2,
            cards_per_type: 2,
        };
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = Game::custom(
            vec![
                vec![Card::Assassin, Card::Assassin],
                vec![Card::Ambassador, Card::Ambassador],
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
        let hand: Vec<Card> = game.get_player_view(0).cards.into();
        let mut tracker = CardsTracker::new(0, &hand, &settings);
        let actions = [
            Action {
                player: 0,
                action_type: ActionType::Income,
            },
            Action {
                player: 1,
                action_type: ActionType::Exchange,
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
                action_type: ActionType::TakeCard,
            },
            Action {
                player: 1,
                action_type: ActionType::TakeCard,
            },
            Action {
                player: 1,
                action_type: ActionType::DropCard(Card::Duke),
            },
            Action {
                player: 1,
                action_type: ActionType::DropCard(Card::Captain),
            },
        ];
        assert_eq!(
            play_actions(&actions, &mut game, &mut tracker, &mut rng),
            Ok(())
        );
        for game_state in tracker.game_states.iter() {
            assert!(
                matches!(game_state.state_type, StateType::Turn { player: 0 }),
                "{:?}",
                game_state.state_type
            );
        }
        assert_eq!(
            tracker.game_states,
            vec![
                GameState {
                    valid: true,
                    state_type: StateType::Turn { player: 0 },
                    player_coins: vec![3, 2],
                    player_hands: vec![1, 2],
                    player_cards_counter: vec![1, 2],
                    player_cards: vec![
                        GamePlayerCards::Player(vec![Card::Assassin]),
                        GamePlayerCards::Opponent(CardCollection {
                            known: vec![],
                            unknown: 2
                        }),
                    ],
                    revealed_cards: vec![Card::Assassin],
                    deck: CardCollection {
                        known: vec![Card::Ambassador],
                        unknown: 5
                    },
                },
                GameState {
                    valid: true,
                    state_type: StateType::Turn { player: 0 },
                    player_coins: vec![3, 2],
                    player_hands: vec![1, 2],
                    player_cards_counter: vec![1, 2],
                    player_cards: vec![
                        GamePlayerCards::Player(vec![Card::Assassin]),
                        GamePlayerCards::Opponent(CardCollection {
                            known: vec![Card::Ambassador],
                            unknown: 1
                        }),
                    ],
                    revealed_cards: vec![Card::Assassin],
                    deck: CardCollection {
                        known: vec![],
                        unknown: 6
                    },
                },
            ]
        );
    }

    fn play_actions<R: Rng>(
        actions: &[Action],
        game: &mut Game,
        tracker: &mut CardsTracker,
        rng: &mut R,
    ) -> Result<(), String> {
        for i in 0..actions.len() {
            game.print();
            let action = &actions[i];
            println!("Play {:?}", action);
            if let Err(e) = game.play(action, rng) {
                return Err(e);
            }
            if action.player == 0 {
                tracker.after_player_action(&game.get_player_view(0), action);
            } else {
                tracker.after_opponent_action(
                    &game.get_player_view(0),
                    &ActionView::from_action(&action),
                );
            }
        }
        game.print();
        Ok(())
    }
}
