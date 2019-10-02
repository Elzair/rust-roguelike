use tcod::colors::*;
use tcod::console::*;

// actual size of the window
const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;

const LIMIT_FPS: i32 = 20; // 20 frames-per-second maximum

/// This is a generic object: the player, a monster, an item, the stairs...
/// It is always represented by a character on screen.
#[derive(Debug)]
struct Object {
    x: i32,
    y: i32,
    char: char,
    color: Color,
}

impl Object {
    pub fn new(x: i32, y: i32, char: char, color: Color) -> Self {
        Object { x, y, char, color }
    }

    // move by the given amount, if the destination is not blocked
    pub fn move_by(&mut self, dx: i32, dy: i32, game: &Game) {
        if !game.map[(self.x + dx) as usize][(self.y + dy) as usize].blocked {
            self.x += dx;
            self.y += dy;
        }
    }

    /// set the color and then draw the character that represents this object at its position
    pub fn draw(&self, con: &mut dyn Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
    }
}

// Size of the map
const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 45;

const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_DARK_GROUND: Color = Color { r: 50, g: 50, b: 150 };

/// A tile of the map and its properties
#[derive(Clone, Copy, Debug)]
struct Tile {
    blocked: bool,
    block_sight: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile {
            blocked: false,
            block_sight: false,
        }
    }

    pub fn wall() -> Self {
        Tile {
            blocked: true,
            block_sight: true,
        }
    }
}

type Map = Vec<Vec<Tile>>;

fn make_map() -> Map {
    // fill map with "unblocked" tiles
    let mut map = vec![vec![Tile::empty(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];

    // place two pillars to test the map
    map[30][22] = Tile::wall();
    map[50][22] = Tile::wall();

    map
}

struct Game {
    map: Map,
}

fn render_all(tcod: &mut Tcod, game: &Game, objects: &[Object]) {
    // Go through all tiles, and set their background color
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let wall = game.map[x as usize][y as usize].block_sight;
            if wall {
                tcod.con.set_char_background(x, y, COLOR_DARK_WALL, BackgroundFlag::Set);
            } else {
                tcod.con.set_char_background(x, y, COLOR_DARK_GROUND, BackgroundFlag::Set);
            }
        }
    }

    // Draw all objects in the list
    for object in objects {
        object.draw(&mut tcod.con);
    }

    // Blit the contents of "con" to the root console
    blit(
        &tcod.con,
        (0, 0),
        (MAP_WIDTH, MAP_HEIGHT),
        &mut tcod.root,
        (0, 0),
        1.0,
        1.0
    );
}

struct Tcod {
    root: Root,
    con: Offscreen,
}

fn handle_keys(tcod: &mut Tcod, game: &Game, player: &mut Object) -> bool {
    use tcod::input::Key;
    use tcod::input::KeyCode::*;

    let key = tcod.root.wait_for_keypress(true);
    match key {
        // Movement keys
        Key { code: Up, .. } => player.move_by(0, -1, game),
        Key { code: Down, .. } => player.move_by(0, 1, game),
        Key { code: Left, .. } => player.move_by(-1, 0, game),
        Key { code: Right, .. } => player.move_by(1, 0, game),

        // Other keys
        Key {
            code: Enter,
            alt: true,
            ..
        } => {
            // Alt+Enter: toggle fullscreen
            let fullscreen = tcod.root.is_fullscreen();
            tcod.root.set_fullscreen(!fullscreen);
        },
        Key { code: Escape, .. } => return true, // exit game
        _ => {}
    }
    false
}

fn main() {
    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust/libtcod tutorial")
        .init();
    
    let con = Offscreen::new(MAP_WIDTH, MAP_HEIGHT);
    
    let mut tcod = Tcod { root, con };

    tcod::system::set_fps(LIMIT_FPS);

    // Create object representing the player
    let player = Object::new(SCREEN_WIDTH/2, SCREEN_HEIGHT/2, '@', WHITE);

    // Create an NPC
    let npc = Object::new(SCREEN_WIDTH / 2 - 5, SCREEN_HEIGHT / 2, '@', YELLOW);

    // list of objects with those two
    let mut objects = [player, npc];

    let game = Game {
        // Generate map (at this point it is not drawn to the screen)
        map: make_map(),
    };

    while !tcod.root.window_closed() {
        // Clear previous frame
        tcod.con.clear();

        // Render the screen
        render_all(&mut tcod, &game, &objects);
        tcod.root.flush();

        // Handle keys and exit game if needed
        let player = &mut objects[0];
        let exit = handle_keys(&mut tcod, &game, player);
        if exit {
            break;
        }
    }
}
