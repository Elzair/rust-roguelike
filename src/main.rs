use std::cmp;

use rand::Rng;

use tcod::colors::*;
use tcod::console::*;
use tcod::input::{self, Event, Key, Mouse};
use tcod::map::{FovAlgorithm, Map as FovMap};

// Actual size of the window
const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;

// Sizes and coordinates relevant for the GUI
const BAR_WIDTH: i32 = 20;
const PANEL_HEIGHT: i32 = 7;
const PANEL_Y: i32 = SCREEN_HEIGHT - PANEL_HEIGHT;

const MSG_X: i32 = BAR_WIDTH + 2;
const MSG_WIDTH: i32 = SCREEN_WIDTH - BAR_WIDTH - 2;
const MSG_HEIGHT: usize = PANEL_HEIGHT as usize - 1;

const LIMIT_FPS: i32 = 20; // 20 frames-per-second maximum

const PLAYER: usize = 0; // Player will always be the first object

/// This is a generic object: the player, a monster, an item, the stairs...
/// It is always represented by a character on screen.
#[derive(Debug)]
struct Object {
    x: i32,
    y: i32,
    char: char,
    color: Color,
    name: String,
    blocks: bool,
    alive: bool,
    fighter: Option<Fighter>,
    ai: Option<Ai>,
}

impl Object {
    pub fn new(x: i32, y: i32, char: char, name: &str, color: Color, blocks: bool) -> Self {
        Object {
            x: x,
            y: y,
            char: char,
            color: color,
            name: name.into(),
            blocks: blocks,
            alive: false,
            fighter: None,
            ai: None,
        }
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    /// set the color and then draw the character that represents this object at its position
    pub fn draw(&self, con: &mut dyn Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
    }

    pub fn take_damage(&mut self, damage: i32, game: &mut Game) {
        // Apply damage if possible
        if let Some(fighter) = self.fighter.as_mut() {
            fighter.hp -= damage;
        }
        // Check for death and call the on_death callback.
        if let Some(fighter) = self.fighter {
            if fighter.hp <= 0 {
                self.alive = false;
                fighter.on_death.callback(self, game);
            }
        }
    }

    pub fn attack(&mut self, target: &mut Object, game: &mut Game) {
        // Use a simple formula for attack damage
        let damage = self.fighter.map_or(0, |f| f.power) - target.fighter.map_or(0, |f| f.defense);
        if damage > 0 {
            // Make target take some damage
            game.messages.add(
                format!(
                    "{} attacks {} for {} hit points.",
                    self.name, target.name, damage
                ),
                WHITE
            );
            target.take_damage(damage, game);
        } else {
            game.messages.add(
                format!(
                    "{} attacks {}, but it has no effect!",
                    self.name, target.name
                ),
                WHITE,
            );
        }
    }

    // move by the given amount, if the destination is not blocked
    pub fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
        let (x, y) = objects[id].pos();
        if !Object::is_blocked(x + dx, y + dy, map, objects) {
            objects[id].set_pos(x + dx, y + dy);
        }
    }

    fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
        // First test map tile
        if map[x as usize][y as usize].blocked {
            return true;
        }

        // Now check for any blocking objects
        objects
            .iter()
            .any(|object| object.blocks && object.pos() == (x, y))
    }

    pub fn player_move_or_attack(dx: i32, dy: i32, game: &mut Game, objects: &mut [Object]) {
        // Coordinates the player is moving to or attacking
        let x = objects[PLAYER].x + dx;
        let y = objects[PLAYER].y + dy;

        // Try to find an attackable object there
        let target_id = objects
            .iter()
            .position(|object| object.fighter.is_some() && object.pos() == (x, y));

        // Attack if target found, move otherwise
        match target_id {
            Some(target_id) => {
                let (monster, player) = mut_two(target_id, PLAYER, objects);
                player.attack(monster, game);
            },
            None => {
                Object::move_by(PLAYER, dx, dy, &game.map, objects);
            }
        }
    }

    pub fn move_towards(id: usize, target_x: i32, target_y: i32, map: &Map, objects: &mut [Object]) {
        // Get vector from this object's tile to the target tile and total distance
        let dx = target_x - objects[id].x;
        let dy = target_y - objects[id].y;
        let distance = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();

        // Normalize vector, then round it and convert it to `i32` to get grid movement
        let dx = (dx as f32 / distance).round() as i32; 
        let dy = (dy as f32 / distance).round() as i32;
        Object::move_by(id, dx, dy, map, objects);
    }

    /// Return the distance to another object
    pub fn distance_to(&self, other: &Object) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        ((dx.pow(2) + dy.pow(2)) as f32).sqrt()
    }

    pub fn ai_take_turn(monster_id: usize, tcod: &Tcod, game: &mut Game, objects: &mut [Object]) {
        // A basic monster takes its turn. If you can see it, it can see you.
        let (monster_x, monster_y) = objects[monster_id].pos();
        if tcod.fov.is_in_fov(monster_x, monster_y) {
            if objects[monster_id].distance_to(&objects[PLAYER]) >= 2.0 {
                // Move towards player if far away
                let (player_x, player_y) = objects[PLAYER].pos();
                Object::move_towards(monster_id, player_x, player_y, &game.map, objects);
            } else if objects[PLAYER].fighter.map_or(false, |f| f.hp > 0) {
                // If monster is close enough (and the player is still alive), ATTACK!
                let (monster, player) = mut_two(monster_id, PLAYER, objects);
                monster.attack(player, game);
            }
        }
    }
}

/// Combat-related component
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fighter {
    max_hp: i32,
    hp: i32,
    defense: i32,
    power: i32,
    on_death: DeathCallback,
}

/// Basic Artificial Intelligence Component
#[derive(Clone, Debug, PartialEq)]
enum Ai {
    Basic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DeathCallback {
    Player,
    Monster,
}

impl DeathCallback {
    fn callback(self, object: &mut Object, game: &mut Game) {
        use DeathCallback::*;
        let callback: fn(&mut Object, game: &mut Game) = match self {
            Player => player_death,
            Monster => monster_death,
        };
        callback(object, game);
    }
}

fn player_death(player: &mut Object, game: &mut Game) {
    // The game ended!
    game.messages.add(format!("You died!"), RED);

    // For added effect, transform the player into a corpse!
    player.char = '%';
    player.color = DARKER_RED;
}

fn monster_death(monster: &mut Object, game: &mut Game) {
    // Transform it into a nasty corpse! It does not block,
    // it cannot be attacked, and it does not move. 
    game.messages.add(format!("{} is dead!", monster.name), ORANGE);
    monster.char = '%';
    monster.color = DARKER_RED;
    monster.blocks = false;
    monster.fighter = None;
    monster.ai = None;
    monster.name = format!("remains of {}", monster.name);
}

// Size of the map
const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 43;

const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_LIGHT_WALL: Color = Color {
    r: 130,
    g: 110,
    b: 50,
};
const COLOR_DARK_GROUND: Color = Color {
    r: 50,
    g: 50,
    b: 150,
};
const COLOR_LIGHT_GROUND: Color = Color {
    r: 200,
    g: 180,
    b: 50,
};

/// A tile of the map and its properties
#[derive(Clone, Copy, Debug)]
struct Tile {
    blocked: bool,
    explored: bool,
    block_sight: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile {
            blocked: false,
            explored: false,
            block_sight: false,
        }
    }

    pub fn wall() -> Self {
        Tile {
            blocked: true,
            explored: false,
            block_sight: true,
        }
    }
}

type Map = Vec<Vec<Tile>>;

// Dungeon Generator Parameters
const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;
const MAX_ROOM_MONSTERS: i32 = 3;

/// A rectangle on the map, used to characterize a room.
#[derive(Clone, Copy, Debug)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect {
            x1: x,
            y1: y,
            x2: x + w,
            y2: y + h,
        }
    }

    pub fn center(&self) -> (i32, i32) {
        let center_x = (self.x1 + self.x2) / 2;
        let center_y = (self.y1 + self.y2) / 2;
        (center_x, center_y)
    }

    pub fn intersects_with(&self, other: &Rect) -> bool {
        // Returns true if this rectangle intersects with another one
        (self.x1 <= other.x2)
            && (self.x2 >= other.x1)
            && (self.y1 <= other.y2)
            && (self.y2 >= other.y1)
    }
}

fn create_room(room: Rect, map: &mut Map) {
    // Go through the tiles in the rectangle and make them passable
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    // horizontal tunnel `min()` and `max()` are used in case `x1 > x2`
    for x in cmp::min(x1, x2)..(cmp::max(x1, x2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    // horizontal tunnel `min()` and `max()` are used in case `y1 > y2`
    for y in cmp::min(y1, y2)..(cmp::max(y1, y2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>) {
    // Choose random number of monsters
    let num_monsters = rand::thread_rng().gen_range(0, MAX_ROOM_MONSTERS + 1);

    for _ in 0..num_monsters {
        // Chose random spot for this monster
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        // Only place monster if tile is not blocked
        if !Object::is_blocked(x, y, map, objects) {
            let mut monster = if rand::random::<f32>() < 0.8 {
                // 80% chance of getting an orc
                // Create an orc
                let mut orc = Object::new(x, y, 'o', "orc", DESATURATED_GREEN, true);
                orc.fighter = Some(Fighter {
                    max_hp: 10,
                    hp: 10,
                    defense: 0,
                    power: 3,
                    on_death: DeathCallback::Monster,
                });
                orc.ai = Some(Ai::Basic);
                orc
            } else {
                let mut troll = Object::new(x, y, 'T', "troll", DARKER_GREEN, true);
                troll.fighter = Some(Fighter {
                    max_hp: 16,
                    hp: 16,
                    defense: 1,
                    power: 4,
                    on_death: DeathCallback::Monster,
                });
                troll.ai = Some(Ai::Basic);
                troll
            };
            
            monster.alive = true;
            objects.push(monster);
        }
    }
}

fn make_map(objects: &mut Vec<Object>) -> Map {
    // fill map with "blocked" tiles
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];

    // Create rooms
    let mut rooms = vec![];

    for _ in 0..MAX_ROOMS {
        // Random width and height
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        // Random position without going out of the boundaries of the map
        let x = rand::thread_rng().gen_range(0, MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, MAP_HEIGHT - h);

        let new_room = Rect::new(x, y, w, h);

        // Run through the other rooms and see if they intersect with this one
        let failed = rooms
            .iter()
            .any(|other_room| new_room.intersects_with(other_room));

        if !failed {
            // The room is valid if there are no intersections

            // "Paint" it to the map's tiles
            create_room(new_room, &mut map);

            // Add some content to this room, such as monsters
            place_objects(new_room, &map, objects);

            // Center coordinates of the new room
            let (new_x, new_y) = new_room.center();

            if rooms.is_empty() {
                // Start player at first created room
                objects[PLAYER].set_pos(new_x, new_y);
            } else {
                // Connect new room to previous room with a tunnel

                // Get center coordinates of the previous room
                let (prev_x, prev_y) = rooms[rooms.len() - 1].center();

                // Toss a coin (random bool value -- either true or false)
                if rand::random() {
                    // First move horizontally, then vertically
                    create_h_tunnel(prev_x, new_x, prev_y, &mut map);
                    create_v_tunnel(prev_y, new_y, new_x, &mut map);
                } else {
                    // First move vertically, then horizontally
                    create_v_tunnel(prev_y, new_y, prev_x, &mut map);
                    create_h_tunnel(prev_x, new_x, new_y, &mut map);
                }
            }

            // Finally, append the new room to `rooms`
            rooms.push(new_room);
        }
    }

    map
}

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic; // default FOV algorithm
const FOV_LIGHT_WALLS: bool = true; // light walls or not
const TORCH_RADIUS: i32 = 10;

struct Game {
    map: Map,
    messages: Messages,
}

struct Messages {
    messages: Vec<(String, Color)>,
}

impl Messages {
    pub fn new() -> Self {
        Self { messages: vec![] }
    }

    /// Add the new message as a tuple, with the text and the color. 
    pub fn add<T: Into<String>>(&mut self, message: T, color: Color) {
        self.messages.push((message.into(), color));
    }

    /// Create a `DoubleEndedIterator` over the messages. 
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &(String, Color)> {
        self.messages.iter()
    }
}

struct Tcod {
    root: Root,
    con: Offscreen,
    panel: Offscreen,
    fov: FovMap,
    key: Key,
    mouse: Mouse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlayerAction {
    TookTurn,
    DidNotTakeTurn,
    Exit,
}

fn handle_keys(tcod: &mut Tcod, game: &mut Game, objects: &mut Vec<Object>) -> PlayerAction {
    use tcod::input::KeyCode::*;

    let player_alive = objects[PLAYER].alive;
    match (tcod.key, tcod.key.text(), player_alive) {
        // Movement keys
        (Key { code: Up, .. }, _, true) => {
            Object::player_move_or_attack(0, -1, game, objects);
            PlayerAction::TookTurn
        },
        (Key { code: Down, .. }, _, true) => {
            Object::player_move_or_attack(0, 1, game, objects);
            PlayerAction::TookTurn
        },
        (Key { code: Left, .. }, _, true) => {
            Object::player_move_or_attack(-1, 0, game, objects);
            PlayerAction::TookTurn
        },
        (Key { code: Right, .. }, _, true) => {
            Object::player_move_or_attack(1, 0, game, objects);
            PlayerAction::TookTurn
        },

        // Other keys
        (Key {
            code: Enter,
            alt: true,
            ..
        }, _, _) => {
            // Alt+Enter: toggle fullscreen
            let fullscreen = tcod.root.is_fullscreen();
            tcod.root.set_fullscreen(!fullscreen);
            PlayerAction::DidNotTakeTurn
        }
        (Key { code: Escape, .. }, _, _) => PlayerAction::Exit, // exit game
        _ => PlayerAction::DidNotTakeTurn,
    }
}

/// Return a string with the names of all objects under the mouse. 
fn get_names_under_mouse(mouse: Mouse, objects: &[Object], fov_map: &FovMap) -> String {
    let (x, y) = (mouse.cx as i32, mouse.cy as i32);

    // Create a list with the names of all objects at the mouse's coordinates and in FOV. 
    let names = objects
        .iter()
        .filter(|obj| obj.pos() == (x, y) && fov_map.is_in_fov(obj.x, obj.y))
        .map(|obj| obj.name.clone())
        .collect::<Vec<_>>();
    
    names.join(", ") // Join the names, separated by commas.
}

fn render_bar(
    panel: &mut Offscreen,
    x: i32,
    y: i32,
    total_width: i32,
    name: &str,
    value: i32,
    maximum: i32,
    bar_color: Color,
    back_color: Color,
) 
{
    // Render a bar (HP, EXP, etc.) First calculate the width of the bar.
    let bar_width = (value as f32 / maximum as f32 * total_width as f32) as i32;

    // Render the background first.
    panel.set_default_background(back_color);
    panel.rect(x, y, total_width, 1, false, BackgroundFlag::Screen);

    // Now render the bar on top.
    panel.set_default_background(bar_color);
    if bar_width > 0 {
        panel.rect(x, y, bar_width, 1, false, BackgroundFlag::Screen);
    }

    // Finally, print some centered text with the values. 
    panel.set_default_foreground(WHITE);
    panel.print_ex(
        x + total_width / 2, 
        y, 
        BackgroundFlag::None, 
        TextAlignment::Center, 
        &format!("{}: {}/{}", name, value, maximum)
    );
}

fn render_all(tcod: &mut Tcod, game: &mut Game, objects: &[Object], fov_recompute: bool) {
    if fov_recompute {
        // Recompute FOV if needed (player moved or something).
        let (px, py) = objects[PLAYER].pos();
        tcod.fov
            .compute_fov(px, py, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    }

    // Go through all tiles, and set their background color.
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let visible = tcod.fov.is_in_fov(x, y);
            let wall = game.map[x as usize][y as usize].block_sight;
            let color = match (visible, wall) {
                // Outside field of view
                (false, true) => COLOR_DARK_WALL,
                (false, false) => COLOR_DARK_GROUND,
                // Inside FOV
                (true, true) => COLOR_LIGHT_WALL,
                (true, false) => COLOR_LIGHT_GROUND,
            };

            let explored = &mut game.map[x as usize][y as usize].explored;
            if visible {
                *explored = true; // Since it is visible, it has been explored.
            }
            if *explored {
                // Only show explored tiles.
                tcod.con
                    .set_char_background(x, y, color, BackgroundFlag::Set);
            }
        }
    }

    // Draw all objects in field of view.
    let mut to_draw: Vec<_> = objects
        .iter()
        .filter(|o| tcod.fov.is_in_fov(o.x, o.y))
        .collect();
    // Sort `to_draw` so that non-blocking objects come first. 
    to_draw.sort_by(|o1, o2| { o1.blocks.cmp(&o2.blocks) });
    // Draw the objects in `to_draw`.
    for object in &to_draw {
        if tcod.fov.is_in_fov(object.x, object.y) {
            object.draw(&mut tcod.con);
        }
    }

    // Blit the contents of "con" to the root console.
    blit(
        &tcod.con,
        (0, 0),
        (MAP_WIDTH, MAP_HEIGHT),
        &mut tcod.root,
        (0, 0),
        1.0,
        1.0,
    );

    // Prepare to render the GUI panel
    tcod.panel.set_default_background(BLACK);
    tcod.panel.clear();

    // Print the game messages, one line at a time. 
    let mut y = MSG_HEIGHT as i32;
    for &(ref msg, color) in game.messages.iter().rev() {
        let msg_height = tcod.panel.get_height_rect(MSG_X, y, MSG_WIDTH, 0, msg);
        y -= msg_height;
        if y < 0 {
            break;
        }
        tcod.panel.set_default_foreground(color);
        tcod.panel.print_rect(MSG_X, y, MSG_WIDTH, 0, msg);
    }

    // Show the player's stats. 
    let hp = objects[PLAYER].fighter.map_or(0, |f| f.hp);
    let max_hp = objects[PLAYER].fighter.map_or(0, |f| f.max_hp);
    render_bar(
        &mut tcod.panel, 
        1, 
        1, 
        BAR_WIDTH, 
        "HP", 
        hp, 
        max_hp, 
        LIGHT_RED, 
        DARKER_RED,
    );

    // Display names of objects under the mouse. 
    tcod.panel.set_default_background(LIGHT_GREY);
    tcod.panel.print_ex(
        1,
        0,
        BackgroundFlag::None,
        TextAlignment::Left,
        get_names_under_mouse(tcod.mouse, objects, &tcod.fov),
    );

    // Blit the contents of `panel` to the root console. 
    blit(
        &tcod.panel,
        (0, 0),
        (SCREEN_WIDTH, PANEL_HEIGHT),
        &mut tcod.root,
        (0, PANEL_Y),
        1.0,
        1.0,
    );
}

fn main() {
    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust/libtcod tutorial")
        .init();

    let mut tcod = Tcod {
        root,
        con: Offscreen::new(MAP_WIDTH, MAP_HEIGHT),
        panel: Offscreen::new(SCREEN_WIDTH, PANEL_HEIGHT),
        fov: FovMap::new(MAP_WIDTH, MAP_HEIGHT),
        key: Default::default(),
        mouse: Default::default(),
    };

    tcod::system::set_fps(LIMIT_FPS);

    // Create object representing the player
    let mut player = Object::new(0, 0, '@', "player", WHITE, true);
    player.alive = true;
    player.fighter = Some(Fighter {
        max_hp: 30,
        hp: 30,
        defense: 2,
        power: 5,
        on_death: DeathCallback::Player,
    });

    // list of objects with those two
    let mut objects = vec![player];

    let mut game = Game {
        // Generate map (at this point it is not drawn to the screen)
        map: make_map(&mut objects),
        messages: Messages::new(),
    };

    // Populate the FOV map, according to the generated map
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            tcod.fov.set(
                x,
                y,
                !game.map[x as usize][y as usize].block_sight,
                !game.map[x as usize][y as usize].blocked,
            );
        }
    }

    // Force FOV "recompute" first time through the game loop
    let mut previous_player_position = (-1, -1);

    // Print a welcome message. 
    game.messages.add(
        "Welcome stranger! Prepare to perish in the Tombs of the Ancient Kings!",
        RED,
    );

    while !tcod.root.window_closed() {
        // Check for mouse or keyboard input
        match input::check_for_event(input::MOUSE | input::KEY_PRESS) {
            Some((_, Event::Mouse(m))) => tcod.mouse = m,
            Some((_, Event::Key(k))) => tcod.key = k,
            _ => tcod.key = Default::default(),
        }

        // Clear previous frame
        tcod.con.clear();

        // Render the screen
        let fov_recompute = previous_player_position != objects[PLAYER].pos();
        render_all(&mut tcod, &mut game, &objects, fov_recompute);
        tcod.root.flush();

        // Handle keys and exit game if needed
        previous_player_position = objects[PLAYER].pos();
        let player_action = handle_keys(&mut tcod, &mut game, &mut objects);
        if player_action == PlayerAction::Exit {
            break;
        }

        // Let monsters take their turn
        if objects[PLAYER].alive && player_action != PlayerAction::DidNotTakeTurn { // NOTE: Should this be `player_action == PlayerAction::TookTurn`?
            for id in 0..objects.len() {
                // Take turn only if object is not player
                if objects[id].ai.is_some() {
                    Object::ai_take_turn(id, &tcod, &mut game, &mut objects);
                }
            }
        }
    }
}

/// Mutably borrow two *separate* elements from the given slice.
/// Panics when the indices are equal or out of bounds. 
pub fn mut_two<T>(first_index: usize, second_index: usize, items: &mut [T]) -> (&mut T, &mut T) {
    assert_ne!(first_index, second_index);
    let split_at_index = cmp::max(first_index, second_index);
    let (first_slice, second_slice) = items.split_at_mut(split_at_index);
    if first_index < second_index {
        (&mut first_slice[first_index], &mut second_slice[0])
    } else {
        (&mut second_slice[0], &mut first_slice[second_index])
    }
}
