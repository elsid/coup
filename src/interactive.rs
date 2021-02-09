use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::str::FromStr;

use itertools::Itertools;
use rand::Rng;
use scan_fmt::parse::ScanError;

use crate::bots::{ActionView, Bot, HonestCarefulRandomBot, RandomBot};
use crate::fsm::{Action, ActionType, Card, CARDS_PER_PLAYER, ConstRng, Deck, play_action, PlayerCards, State, StateType};
use crate::game::{ALL_CARDS, get_available_actions, INITIAL_COINS, PlayerView, Settings};
use crate::run::BotType;

#[derive(Debug)]
enum Command {
    Help,
    Quit,
    Set(SetCommand),
    NamePlayer {
        index: usize,
        name: String,
    },
    Add(Card),
    Remove(Card),
    Start,
    Play(GameAction),
    Undo,
    State,
    Available,
    Bot(BotCommand),
}

#[derive(Debug)]
enum SetCommand {
    PlayersNumber(usize),
    CardsPerType(usize),
    BotType(BotType),
    PlayerIndex(usize),
}

#[derive(Debug)]
enum BotCommand {
    SuggestActions,
    GetAction,
    Custom(String),
}

#[derive(Debug)]
struct GameAction {
    player: String,
    action_type: GameActionType,
}

#[derive(Debug)]
pub enum GameActionType {
    Income,
    ForeignAid,
    Coup(String),
    Tax,
    Assassinate(String),
    Exchange,
    Steal(String),
    Block(Card),
    PassChallenge,
    PassBlock,
    Challenge,
    ShowCard(Card),
    RevealCard(Card),
    DropCard(Card),
    TakeCard(Card),
    ShuffleDeck,
}

pub fn run_interactive_game() {
    let mut settings = Settings {
        players_number: 6,
        cards_per_type: 2,
    };
    let mut bot_type = BotType::HonestCarefulRandom;
    let mut player_index = 0;
    let mut player_cards = Vec::with_capacity(2);
    let mut custom_player_names: HashMap<usize, String> = HashMap::new();
    println!("Use default settings:");
    println!("players_number: {}", settings.players_number);
    println!("cards_per_type: {}", settings.cards_per_type);
    println!("player_index: {}", player_index);
    println!("bot_type: {:?}", bot_type);
    loop {
        match read_command() {
            Command::Help => println!("{}", HELP),
            Command::Quit => break,
            Command::Set(set) => {
                match set {
                    SetCommand::PlayersNumber(v) => settings.players_number = v,
                    SetCommand::CardsPerType(v) => settings.cards_per_type = v,
                    SetCommand::BotType(v) => bot_type = v,
                    SetCommand::PlayerIndex(v) => player_index = v,
                }
            }
            Command::NamePlayer { index, name } => {
                if index >= settings.players_number {
                    println!("Player index is not applicable for current number of players: {}", settings.players_number);
                    continue;
                }
                custom_player_names.insert(index, name);
            }
            Command::Add(card) => {
                if player_cards.len() >= CARDS_PER_PLAYER {
                    println!("Can't add more than {} cards", CARDS_PER_PLAYER);
                    continue;
                }
                player_cards.push(card);
            }
            Command::Remove(card) => {
                if player_cards.is_empty() {
                    println!("Can't add more than {} cards", CARDS_PER_PLAYER);
                    continue;
                }
                let index = player_cards.iter()
                    .find_position(|v| **v == card)
                    .map(|(i, _)| i);
                if let Some(index) = index {
                    player_cards.remove(index);
                } else {
                    println!("Don't have {:?} card", card);
                }
            }
            Command::Start => {
                if player_cards.len() != CARDS_PER_PLAYER {
                    println!("Need to add {} more card(s)", CARDS_PER_PLAYER - player_cards.len());
                    continue;
                }
                if settings.cards_per_type * ALL_CARDS.len() < settings.players_number * CARDS_PER_PLAYER {
                    println!(
                        "Not enough cards for all players: need at least {} cards per type for {} players",
                        (settings.players_number * CARDS_PER_PLAYER) / ALL_CARDS.len(),
                        settings.players_number
                    );
                    continue;
                }
                let game_state = make_initial_game_state(&settings, player_index, player_cards.clone());
                let player_names: Vec<String> = (0..settings.players_number)
                    .map(|index| {
                        if let Some(v) = custom_player_names.get(&index) {
                            v.clone()
                        } else {
                            if index == player_index {
                                String::from("me")
                            } else {
                                format!("{}", index)
                            }
                        }
                    })
                    .collect();
                println!("Start game with initial state:");
                println!("players_number: {}", settings.players_number);
                println!("cards_per_type: {}", settings.cards_per_type);
                println!("bot_type: {:?}", bot_type);
                print_state(&game_state, &player_names);
                match bot_type {
                    BotType::Random => {
                        let bot = RandomBot::new(&game_state.player_view());
                        interactive_with_bot(&player_names, game_state, bot);
                    }
                    BotType::HonestCarefulRandom => {
                        let bot = HonestCarefulRandomBot::new(&game_state.player_view(), &settings);
                        interactive_with_bot(&player_names, game_state, bot);
                    }
                }
                break;
            }
            _ => println!("Invalid command"),
        }
    }
}

fn read_command() -> Command {
    loop {
        print!("> ");
        std::io::stdout().flush().unwrap();
        let mut line = String::new();
        if let Err(e) = std::io::stdin().lock().read_line(&mut line) {
            println!("{}", e);
            continue;
        }
        if line.is_empty() {
            return Command::Quit;
        }
        print!("{}", line);
        match parse_command(&line) {
            Ok(v) => return v,
            Err(e) => println!("{}", e),
        }
    }
}

fn parse_command(line: &str) -> Result<Command, ScanError> {
    let name = scan_fmt!(line, "{}", String)?;
    match name.as_str() {
        "help" => Ok(Command::Help),
        "quit" => Ok(Command::Quit),
        "set" => {
            Ok(Command::Set(match scan_fmt!(line, "set {}", String)?.as_str() {
                "players_number" => {
                    SetCommand::PlayersNumber(scan_fmt!(line, "set players_number {d}", usize)?)
                }
                "cards_per_type" => {
                    SetCommand::CardsPerType(scan_fmt!(line, "set cards_per_type {d}", usize)?)
                }
                "bot_type" => {
                    SetCommand::BotType(scan(scan_fmt!(line, "set bot_type {}", String)?)?)
                }
                "player" => {
                    SetCommand::PlayerIndex(scan_fmt!(line, "set player {}", usize)?)
                }
                v => return Err(ScanError(format!("invalid set command param: {}", v))),
            }))
        }
        "name" => {
            let (index, name) = scan_fmt!(line, "name {d} {}", usize, String)?;
            Ok(Command::NamePlayer { index, name })
        }
        "add" => Ok(Command::Add(scan(scan_fmt!(line, "add {}", String)?)?)),
        "rm" => Ok(Command::Remove(scan(scan_fmt!(line, "rm {}", String)?)?)),
        "start" => Ok(Command::Start),
        "play" => {
            let player = scan_fmt!(line, "play {}", String)?;
            let sub = get_tail(player.len(), get_tail(4, line));
            let action_type = match scan_fmt!(sub, "{}", String)?.as_str() {
                "income" => GameActionType::Income,
                "coup" => GameActionType::Coup(scan_fmt!(sub, "coup {}", String)?),
                "foreign_aid" | "aid" => GameActionType::ForeignAid,
                "tax" => GameActionType::Tax,
                "assassinate" => GameActionType::Assassinate(scan_fmt!(sub, "assassinate {}", String)?),
                "kill" => GameActionType::Assassinate(scan_fmt!(sub, "kill {}", String)?),
                "exchange" => GameActionType::Exchange,
                "steal" => GameActionType::Steal(scan_fmt!(sub, "steal {}", String)?),
                "block" => GameActionType::Block(scan(scan_fmt!(sub, "block {}", String)?)?),
                "pass_challenge" | "pass_c" => GameActionType::PassChallenge,
                "pass_block" | "pass_b" => GameActionType::PassBlock,
                "challenge" => GameActionType::Challenge,
                "show" => GameActionType::ShowCard(scan(scan_fmt!(sub, "show {}", String)?)?),
                "reveal" => GameActionType::RevealCard(scan(scan_fmt!(sub, "reveal {}", String)?)?),
                "drop" => GameActionType::DropCard(scan(scan_fmt!(sub, "drop {}", String)?)?),
                "take" => GameActionType::TakeCard(scan(scan_fmt!(sub, "take {}", String)?)?),
                "shuffle" => GameActionType::ShuffleDeck,
                v => return Err(ScanError(format!("invalid action type: {}", v))),
            };
            Ok(Command::Play(GameAction { player, action_type }))
        }
        "undo" => Ok(Command::Undo),
        "state" => Ok(Command::State),
        "available" => Ok(Command::Available),
        "bot" => {
            Ok(Command::Bot(match scan_fmt!(line, "bot {}", String)?.as_str() {
                "suggest" => BotCommand::SuggestActions,
                "get" => BotCommand::GetAction,
                "custom" => BotCommand::Custom(get_tail(name.len(), get_tail(3, &line)).into()),
                v => return Err(ScanError(format!("invalid bot command: {}", v))),
            }))
        }
        v => Err(ScanError(format!("invalid command name: {}", v))),
    }
}

fn get_tail(skip: usize, line: &str) -> &str {
    let spaces = line.bytes()
        .skip(skip)
        .find_position(|v| *v != b' ')
        .map(|(i, _)| i)
        .unwrap();
    &line[skip + spaces..line.len()]
}

fn make_initial_game_state(settings: &Settings, player: usize, cards: Vec<Card>) -> GameState {
    let mut player_cards: Vec<GamePlayerCards> = (0..settings.players_number)
        .map(|_| GamePlayerCards::Opponent(CARDS_PER_PLAYER))
        .collect();
    player_cards[player] = GamePlayerCards::Player(cards);
    GameState {
        step: 0,
        turn: 0,
        round: 0,
        player,
        state_type: StateType::Turn { player: 0 },
        player_coins: std::iter::repeat(INITIAL_COINS).take(settings.players_number).collect(),
        player_hands: std::iter::repeat(CARDS_PER_PLAYER).take(settings.players_number).collect(),
        player_cards_counter: std::iter::repeat(CARDS_PER_PLAYER).take(settings.players_number).collect(),
        player_cards,
        revealed_cards: Vec::with_capacity(settings.cards_per_type * ALL_CARDS.len()),
        deck: GameDeck { size: settings.cards_per_type * ALL_CARDS.len() - CARDS_PER_PLAYER * settings.players_number },
    }
}

#[derive(Debug, Clone)]
struct GameState {
    step: usize,
    turn: usize,
    round: usize,
    player: usize,
    state_type: StateType,
    player_coins: Vec<usize>,
    player_hands: Vec<usize>,
    player_cards_counter: Vec<usize>,
    player_cards: Vec<GamePlayerCards>,
    revealed_cards: Vec<Card>,
    deck: GameDeck,
}

impl GameState {
    fn player_view(&self) -> PlayerView {
        PlayerView {
            step: self.step,
            turn: self.turn,
            round: self.round,
            player: self.player,
            coins: self.player_coins[self.player],
            cards: if let GamePlayerCards::Player(cards) = &self.player_cards[self.player] {
                cards
            } else {
                panic!("Invalid player cards kind: {:?}", self.player_cards[self.player]);
            },
            state_type: &self.state_type,
            player_coins: &self.player_coins,
            player_hands: &self.player_hands,
            player_cards: &self.player_cards_counter,
            revealed_cards: &self.revealed_cards,
            deck: self.deck.size,
        }
    }

    fn with_default<F>(&mut self, mut f: F) -> Result<(), String>
        where F: FnMut(&mut State<GamePlayerCards, GameDeck>) -> Result<(), String> {
        f(&mut State {
            state_type: &mut self.state_type,
            player_coins: &mut self.player_coins,
            player_hands: &mut self.player_hands,
            player_cards_counter: &mut self.player_cards_counter,
            player_cards: &mut self.player_cards,
            deck: &mut self.deck,
            revealed_cards: &mut self.revealed_cards,
        })?;
        self.advance();
        Ok(())
    }

    fn with_pop_deck<F>(&mut self, card: Card, mut f: F) -> Result<(), String>
        where F: FnMut(&mut State<GamePlayerCards, PopGameDeck>) -> Result<(), String> {
        f(&mut State {
            state_type: &mut self.state_type,
            player_coins: &mut self.player_coins,
            player_hands: &mut self.player_hands,
            player_cards_counter: &mut self.player_cards_counter,
            player_cards: &mut self.player_cards,
            deck: &mut PopGameDeck { deck: &mut self.deck, card },
            revealed_cards: &mut self.revealed_cards,
        })?;
        self.advance();
        Ok(())
    }

    fn advance(&mut self) {
        self.step += 1;
        if let StateType::Turn { player } = &self.state_type {
            self.turn += 1;
            if self.player >= *player {
                self.round += 1;
            }
        }
    }
}

#[derive(Debug, Clone)]
enum GamePlayerCards {
    Opponent(usize),
    Player(Vec<Card>),
}

impl PlayerCards for GamePlayerCards {
    fn has_card(&self, card: Card) -> bool {
        match self {
            GamePlayerCards::Player(cards) => cards.contains(&card),
            GamePlayerCards::Opponent(count) => *count > 0,
        }
    }

    fn count(&self) -> usize {
        match self {
            GamePlayerCards::Player(cards) => cards.len(),
            GamePlayerCards::Opponent(count) => *count,
        }
    }

    fn add_card(&mut self, card: Card) {
        match self {
            GamePlayerCards::Player(cards) => {
                cards.push(card);
                cards.sort();
            }
            GamePlayerCards::Opponent(count) => *count += 1,
        }
    }

    fn drop_card(&mut self, card: Card) {
        match self {
            GamePlayerCards::Player(cards) => cards.drop_card(card),
            GamePlayerCards::Opponent(count) => *count -= 1,
        }
    }
}

#[derive(Debug, Clone)]
struct GameDeck {
    size: usize,
}

impl Deck for GameDeck {
    fn count(&self) -> usize { self.size }

    fn pop_card(&mut self) -> Card { unimplemented!() }

    fn push_card(&mut self, _: Card) {
        self.size += 1;
    }

    fn shuffle<R: Rng>(&mut self, _: &mut R) {}
}

struct PopGameDeck<'a> {
    card: Card,
    deck: &'a mut GameDeck,
}

impl<'a> Deck for PopGameDeck<'a> {
    fn count(&self) -> usize { self.deck.size }

    fn pop_card(&mut self) -> Card {
        self.deck.size -= 1;
        self.card
    }

    fn push_card(&mut self, _: Card) { unimplemented!() }

    fn shuffle<R: Rng>(&mut self, _: &mut R) {}
}

fn interactive_with_bot<B: Bot + Sized + Clone>(player_names: &[String], mut game_state: GameState, mut bot: B) {
    let mut history: Vec<(GameState, B)> = Vec::new();
    loop {
        match read_command() {
            Command::Help => println!("{}", HELP),
            Command::Quit => break,
            Command::Play(game_action) => {
                history.push((game_state.clone(), bot.clone()));
                if let Err(e) = handle_game_action(&game_action, player_names, &mut game_state, &mut bot) {
                    println!("{}", e);
                    continue;
                }
            }
            Command::Undo => {
                if let Some((prev_game_state, prev_bot)) = history.pop() {
                    game_state = prev_game_state;
                    bot = prev_bot;
                } else {
                    println!("Nothing to undo");
                }
            }
            Command::State => print_state(&game_state, player_names),
            Command::Available => {
                let available_actions = get_available_actions(&game_state.state_type, &game_state.player_coins, &game_state.player_hands);
                for action in available_actions {
                    println!("{}", to_game_command(&action, player_names));
                }
            }
            Command::Bot(bot_command) => {
                let available_actions = get_available_actions(&game_state.state_type, &game_state.player_coins, &game_state.player_hands).into_iter()
                    .filter(|action| action.player == game_state.player)
                    .collect();
                match bot_command {
                    BotCommand::SuggestActions => {
                        for action in bot.suggest_actions(&game_state.player_view(), &available_actions) {
                            println!("{}", to_game_command(action, player_names));
                        }
                    }
                    BotCommand::GetAction => {
                        let action = bot.get_action(&game_state.player_view(), &available_actions);
                        println!("{}", to_game_command(&action, player_names));
                    }
                    BotCommand::Custom(command) => bot.query(&command),
                }
            }
            _ => (),
        }
    }
}

fn handle_game_action<B: Bot>(game_action: &GameAction, player_names: &[String], game_state: &mut GameState, bot: &mut B) -> Result<(), String> {
    let player = get_player_index(&game_action.player, &player_names)?;
    let action_type = match &game_action.action_type {
        GameActionType::Income => ActionType::Income,
        GameActionType::ForeignAid => ActionType::ForeignAid,
        GameActionType::Coup(target) => {
            ActionType::Coup(get_player_index(target, player_names)?)
        }
        GameActionType::Tax => ActionType::Tax,
        GameActionType::Assassinate(target) => {
            ActionType::Assassinate(get_player_index(target, player_names)?)
        }
        GameActionType::Exchange => ActionType::Exchange,
        GameActionType::Steal(target) => {
            ActionType::Steal(get_player_index(target, player_names)?)
        }
        GameActionType::Block(card) => {
            match card {
                Card::Duke => ActionType::BlockForeignAid,
                Card::Contessa => ActionType::BlockAssassination,
                Card::Ambassador | Card::Captain => ActionType::BlockSteal(*card),
                _ => return Err(format!("invalid card to block: {:?}", card))
            }
        }
        GameActionType::PassChallenge => ActionType::PassChallenge,
        GameActionType::PassBlock => ActionType::PassBlock,
        GameActionType::Challenge => ActionType::Challenge,
        GameActionType::ShowCard(card) => ActionType::ShowCard(*card),
        GameActionType::RevealCard(card) => ActionType::RevealCard(*card),
        GameActionType::DropCard(card) => ActionType::DropCard(*card),
        GameActionType::ShuffleDeck => ActionType::ShuffleDeck,
        GameActionType::TakeCard(card) => {
            if player == game_state.player && matches!(card, Card::Unknown) {
                return Err(String::from("Player can't take unknown card"));
            }
            let action = Action { player, action_type: ActionType::TakeCard };
            game_state.with_pop_deck(*card, |state| play(&action, state))?;
            if game_state.player == action.player {
                bot.after_player_action(&game_state.player_view(), &action);
            } else {
                bot.after_opponent_action(&game_state.player_view(), &ActionView::from_action(&action));
            }
            return Ok(());
        }
    };
    let action = Action { player, action_type };
    game_state.with_default(|state| play(&action, state))?;
    if game_state.player == action.player {
        bot.after_player_action(&game_state.player_view(), &action);
    } else {
        bot.after_opponent_action(&game_state.player_view(), &ActionView::from_action(&action));
    }
    Ok(())
}

fn play<'a, P: PlayerCards + Sized, D: Deck>(action: &Action, state: &mut State<'a, P, D>) -> Result<(), String> {
    if let Err(e) = play_action(action, state, &mut ConstRng) {
        Err(format!("Failed to play action: {:?}", e))
    } else {
        Ok(())
    }
}

fn get_player_index(name: &String, player_names: &[String]) -> Result<usize, String> {
    player_names.iter()
        .find_position(|v| **v == *name)
        .map(|(i, _)| Ok(i))
        .unwrap_or_else(|| Err(format!("invalid player name: {}", name)))
}

fn to_game_command(action: &Action, player_names: &[String]) -> String {
    match &action.action_type {
        ActionType::Income => format!("play {} income", player_names[action.player]),
        ActionType::ForeignAid => format!("play {} foreign_aid", player_names[action.player]),
        ActionType::Coup(target) => format!("play {} coup {}", player_names[action.player], player_names[*target]),
        ActionType::Tax => format!("play {} tax", player_names[action.player]),
        ActionType::Assassinate(target) => format!("play {} assassinate {}", player_names[action.player], player_names[*target]),
        ActionType::Exchange => format!("play {} exchange", player_names[action.player]),
        ActionType::Steal(target) => format!("play {} steal {}", player_names[action.player], player_names[*target]),
        ActionType::BlockForeignAid => format!("play {} block {:?}", player_names[action.player], Card::Duke),
        ActionType::BlockAssassination => format!("play {} block {:?}", player_names[action.player], Card::Contessa),
        ActionType::BlockSteal(card) => format!("play {} block {:?}", player_names[action.player], *card),
        ActionType::PassChallenge => format!("play {} pass_challenge", player_names[action.player]),
        ActionType::PassBlock => format!("play {} pass_block", player_names[action.player]),
        ActionType::Challenge => format!("play {} challenge", player_names[action.player]),
        ActionType::ShowCard(card) => format!("play {} show {:?}", player_names[action.player], *card),
        ActionType::RevealCard(card) => format!("play {} reveal {:?}", player_names[action.player], *card),
        ActionType::DropCard(card) => format!("play {} drop {:?}", player_names[action.player], *card),
        ActionType::TakeCard => format!("play {} take card", player_names[action.player]),
        ActionType::ShuffleDeck => format!("play {} shuffle", player_names[action.player]),
    }
}

fn scan<T: FromStr<Err=String>>(value: String) -> Result<T, ScanError> {
    match T::from_str(&value) {
        Ok(v) => Ok(v),
        Err(e) => Err(ScanError(e)),
    }
}

fn print_state(game_state: &GameState, player_names: &[String]) {
    println!("step: {:?}", game_state.step);
    println!("turn: {:?}", game_state.turn);
    println!("round: {:?}", game_state.round);
    println!("state_type: {:?}", game_state.state_type);
    println!("player_index: {}", game_state.player);
    println!("deck size: {}", game_state.deck.size);
    println!("players:");
    for i in 0..player_names.len() {
        print!("{}) {} coins={} ", i, player_names[i], game_state.player_coins[i]);
        match &game_state.player_cards[i] {
            GamePlayerCards::Player(cards) => println!("cards={:?}", cards),
            GamePlayerCards::Opponent(count) => println!("cards={}", count),
        }
    }
    std::io::stdout().flush().unwrap();
}

const HELP: &'static str = r#"
Commands:
help - show this message
quit - stop the game and exit the process
set players_number <number> - set number of players before the game starts
set cards_per_type <number> - set how much of each card is present before the game starts
set bot_type <name> - set a bot type with given name before the game starts
set player <index> - set which player you are going to play before the game starts
name <index> <string> - set custom name for given player before the game starts
add <name> - add a card with given name to the player hand before the game starts
rm <name> - remove a card with given name from the player hand before the game starts
start - start the game with current settings
play <player_name> income - given player takes one coin at the game turn start
play <player_name> coup <target> - given player pays 7 coins and performs a coup for the target player at the game turn start
play <player_name> foreign_aid|aid - given player starts foreign aid at the game turn start
play <player_name> tax - given player claims Duke and starts to get a tax at the game turn start
play <player_name> assassinate|kill <target> - given player pays 3 coins and starts an assassination for the target player at the game turn start
play <player_name> steal <target> - given player starts stealing for the target player at the game turn start
play <player_name> challenge - given player starts a challenge against a claim of other player, allowed only after any action or block that require a claim
play <player_name> pass_challenge|pass_c - given player considers that none challenges are started after it claimed a card
play <player_name> show <card> - given player shows a card to prove a claim when challenged and puts the card into a deck
play <player_name> reveal <card> - given player reveals the card to lose the influence, now the card is visible to everyone
play <player_name> block <card> - given player blocks any action in progress claiming to have given card, allowed only after passed challenge or when unchallangeable action is in progress
play <player_name> pass_block|pass_b - given player considers that no more blocks are going to happen for its current action
play <player_name> shuffle - given player shuffles a deck before taking a card after showing a card
play <player_name> take <card> - given player takes the card from a deck to get a new card instead of showed one or when does exchange
play <player_name> drop <card> - given player puts the card into a deck to finish the exchange action
undo - undo last game action
state - print current game state
avaialble - print all avaialble actions for all players at the current game state
bot suggest - print all suggested actions by current bot at the current game state
bot get - print action that would be used by a bot at the current game state
bot custom <query> - send a custom query to a bot, implementation depends on the bot type

Cards:
Unknown|unknown - use for opponents take and drop actions, indicates that only that player can see the card
Assassin|assassin - can assassinate other players to reduce influence by forcing to reveal a card
Ambassador|ambassador - can exchange at most 2 cards from deck and block stealing
Captain|captain - can steal 2 coins at most from other players and block stealing
Contessa|contessa - can block assassination
Duke|duke - can get tax and block foreign aid

Target:
Only other players can be targeted. Players with no cards can't be targeted.
"#;
