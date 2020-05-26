use tcod::colors::*;
use tcod::console::*;
use tcod::map::{FovAlgorithm, Map as FovMap};
use std::cmp;
use rand::Rng;

// Window size
const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;

// Map size
const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 45;

const LIMIT_FPS: i32 = 60;

const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_LIGHT_WALL: Color = Color { r: 130, g: 110, b: 50 };
const COLOR_DARK_GROUND: Color = Color { r: 50, g: 50, b: 100 };
const COLOR_LIGHT_GROUND: Color = Color { r: 200, g: 180, b: 50};

const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

const MAX_ROOM_MONSTERS: i32 = 3;

const PLAYER: usize = 0;

struct Tcod {
    root: Root,
    con: Offscreen,
    fov: FovMap,
}

type Map = Vec<Vec<Tile>>;

struct Game {
    map: Map,
}

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

#[derive(Clone, Copy, Debug)]
struct Rect {
    x1: i32,    
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self{
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
        (self.x1 <= other.x2)
            && (self.x2 >= other.x1)
            && (self.y1 <= other.y2)
            && (self.y2 >= other.y1)
    }
}

#[derive(Debug)]
struct Object {
    x: i32,
    y: i32,
    char: char,
    color: Color,
    name: String,
    blocks: bool,
    alive: bool,
}

impl Object {
    pub fn new(x: i32, y: i32, char: char, color: Color, name: &str, blocks: bool) -> Self {
        Object {
            x,
            y,
            char,
            color,
            name: name.into(),
            blocks,
            alive: false,
        }
    }

    pub fn move_by(&mut self, dx: i32, dy: i32, game: &Game) {
        if !game.map[(self.x + dx) as usize][(self.y + dy) as usize].blocked {
            self.x += dx;
            self.y += dy;
        }       
    }

    pub fn draw(&self, con: &mut dyn Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }
}

fn is_blocked(x: i32, y: i32, map: &Map, objects: &mut [Object]) -> bool {
    if map[x as usize][y as usize].blocked {
        return true
    }
    
    objects
        .iter()
        .any(|object| object.blocks && object.pos() == (x,y))
}

fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    let (x, y) = objects[id].pos();
    if !is_blocked(x + dx, y + dy, map, objects) {
        objects[id].set_pos(x + dx, y + dy)
    }
}

fn create_room(room: Rect, map: &mut Map) {
    // make all tiles in a room passable
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>) {
    let num_monsters = rand::thread_rng().gen_range(0, MAX_ROOM_MONSTERS + 1);

    for _ in 0..num_monsters {
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        if !is_blocked(x, y, map, objects) {
            let mut monster = if rand::random::<f32>() < 0.8 {
                // 80% chance to create an orc
                Object::new(x, y, 'o', tcod::colors::DESATURATED_GREEN, "orc", true)
            } else {
                Object::new(x, y, 'T', tcod::colors::DARKER_GREEN, "troll", true)
            };
            monster.alive = true;
            objects.push(monster);
        };        
    }
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    // horizontal tunnel -> min() and max() are used in case x1 > x2
    for x in cmp::min(x1, x2)..(cmp::max(x1, x2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    // vertical tunnel
    for y in cmp::min(y1, y2)..(cmp::max(y1, y2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn make_map(objects: &mut Vec<Object>) -> Map {
    // fill map with blocked tiles
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];

    // let room1 = Rect::new(20, 15, 10, 15);
    // let room2 = Rect::new(50, 15, 10, 15);
    // create_room(room1, &mut map);
    // create_room(room2, &mut map);
    // create_h_tunnel(25, 55, 23, &mut map);

    let mut rooms = vec![];

    for _ in 0..MAX_ROOMS {
        // generate random width and height values
        let width = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let height = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);

        // random positions within bounds
        let new_x = rand::thread_rng().gen_range(0, MAP_WIDTH - width);
        let new_y = rand::thread_rng().gen_range(0, MAP_HEIGHT - height);

        let new_room = Rect::new(new_x, new_y, width, height);

        // check for intersections with rooms
        let failed = rooms
            .iter()
            .any(|other_room| new_room.intersects_with(other_room));

        if !failed {
            create_room(new_room, &mut map);

            place_objects(new_room, &map, objects,);

            let (cx, cy) = new_room.center();

            
            if rooms.is_empty() {
                // place the player in the first room
                objects[PLAYER].set_pos(new_x, new_y);
            } else {
                // connect all future rooms to the previous one

                let(prev_cx, prev_cy) = rooms[rooms.len() - 1].center();

                // some rooms are not in line horizontally or vertically
                // they need a horizontal tunnel and a vertical
                // order as to which is built first is decided by coin toss
                if rand::random() {
                    create_h_tunnel(prev_cx, cx, prev_cy, &mut map);
                    create_v_tunnel(prev_cy, cy, cx, &mut map);
                } else {
                    create_v_tunnel(prev_cy, cy, prev_cx, &mut map);
                    create_h_tunnel(prev_cx, cx, cy, &mut map);
                }
            }

            rooms.push(new_room);
        }
    }

    map
}

fn render_all(tcod: &mut Tcod, game: &mut Game, objects: &[Object], fov_recompute: bool) {
    if fov_recompute {
        let player = &objects[PLAYER];
        tcod.fov
            .compute_fov(player.x, player.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    }

    // set background colour for all tiles
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let visible = tcod.fov.is_in_fov(x, y);
            let wall = game.map[x as usize][y as usize].block_sight;

            let colour = match(visible, wall) {
                (false, true) => COLOR_DARK_WALL,
                (false, false) => COLOR_DARK_GROUND,
                (true, true) => COLOR_LIGHT_WALL,
                (true, false) => COLOR_LIGHT_GROUND,
            };

            let explored = &mut game.map[x as usize][y as usize].explored;
            if visible {
                *explored = true;
            }
            if *explored {
                tcod.con
                    .set_char_background(x, y, colour, BackgroundFlag::Set);
            }
        }
    }

    for object in objects {
        if tcod.fov.is_in_fov(object.x, object.y) {
            object.draw(&mut tcod.con)
        }        
    }

    blit (
        &tcod.con,
        (0,0),
        (MAP_WIDTH, MAP_HEIGHT),
        &mut tcod.root,
        (0,0),
        1.0,
        1.0,
    )
}

fn player_move_or_attack(dx: i32, dy: i32, game: &Game, objects: &mut [Object]) {
    let x = objects[PLAYER].x + dx;
    let y = objects[PLAYER].y + dy;

    // attempt to find attackable object at new location
    let target_id = objects.iter().position(|object| object.pos() == (x,y));

    // attack if target otherwise move
    match target_id {
        Some(target_id) => {
            println!(
                "The {} laughs at your puny effort to attack him!",
                objects[target_id].name
            );
        }
        None => {
            move_by(PLAYER, dx, dy, &game.map, objects);
        }
    }
}

fn handle_keys(tcod: &mut Tcod, game: &Game, objects: &mut Vec<Object>) -> PlayerAction {
    use tcod::input::Key;
    use tcod::input::KeyCode::*;
    use PlayerAction::*;

    let key = tcod.root.wait_for_keypress(true);
    let player_alive = objects[PLAYER].alive;

    match (key, key.text(), player_alive) {
        (
            Key {
                code: Enter,
                alt: true,
                ..
            },
            _,
            _,
        ) => {
            let fullscreen = tcod.root.is_fullscreen();
            tcod.root.set_fullscreen(!fullscreen);
            DidntTakeTurn
        }
        (Key { code: Escape, ..}, _, _) => Exit,

        (Key { code: Up, ..}, _, true) => {
            player_move_or_attack(0, -1, game, objects);
            TookTurn
        }
        (Key { code: Down, ..}, _, true) => {
            player_move_or_attack(0, 1, game, objects);
            TookTurn
        }
        (Key { code: Left, ..}, _, true) => {
            player_move_or_attack(-1, 0, game, objects);
            TookTurn
        }
        (Key { code: Right, ..}, _, true) => {
            player_move_or_attack(1, 0, game, objects);
            TookTurn
        }

        _ => DidntTakeTurn
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlayerAction {
    TookTurn, 
    DidntTakeTurn,
    Exit,
}

fn main() {
    tcod::system::set_fps(LIMIT_FPS);

    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("roguelike")
        .init();

    let mut tcod = Tcod {
        root,
        con: Offscreen::new(MAP_WIDTH, MAP_HEIGHT),
        fov: FovMap::new(MAP_WIDTH, MAP_HEIGHT),     
    };

    let mut player = Object::new(0, 0, '@', WHITE, "player", true);
    player.alive = true;

    let mut objects = vec![player];

    let mut game = Game {
        map: make_map(&mut objects),
    };

    // create FOV map
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            tcod.fov.set(
                x,
                y,
                !game.map[x as usize][y as usize].block_sight,
                !game.map[x as usize][y as usize].blocked
            )
        }
    }

    // force fov recompute for first loop
    let mut previous_player_position = (-1, -1);

    while !tcod.root.window_closed() {
        tcod.con.clear();

        // only need to recompute fov if the player has changed position
        let fov_recompute = previous_player_position != (objects[PLAYER].pos());
        render_all(&mut tcod, &mut game, &objects, fov_recompute);

        tcod.root.flush();

        // handle keys and potentially exit
        previous_player_position = objects[PLAYER].pos();
        let player_action = handle_keys(&mut tcod, &game, &mut objects);
        if player_action == PlayerAction::Exit {
            break;
        }

    }
}