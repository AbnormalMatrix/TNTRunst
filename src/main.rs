use std::net::SocketAddr;
use std::thread;
use std::time;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashSet;
use valence::prelude::*;


pub fn main() -> ShutdownResult {
    tracing_subscriber::fmt().init();

    valence::start_server(Game::default(), ServerState::default())
}

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
struct SteppedOnBlock {
    position: BlockPos,
    removal_time: SystemTime,
}


#[derive(Default)]
struct Game {}

#[derive(Default)]
struct ClientState {
    entity_id: EntityId,
}

#[derive(Default)]
struct ServerState {
    world: WorldId,
    blocks_stepped_on: HashSet<SteppedOnBlock>,
}

const FLOOR_Y: i32 = 1;
const PLATFORM_X: i32 = 15;
const PLATFORM_Z: i32 = 15;
const SPAWN_POS: Vec3<f64> = Vec3::new(1.5, 2.0, 1.5);

#[async_trait]
impl Config for Game {
    type ServerState = ServerState;
    type ClientState = ClientState;
    type EntityState = ();
    type WorldState = ();
    type ChunkState = ();
    type PlayerListState = ();

    fn max_connections(&self) -> usize {
        64
    }

    async fn server_list_ping(
        &self,
        _server: &SharedServer<Self>,
        _remote_addr: SocketAddr,
        _protocol_version: i32,
    ) -> ServerListPing {
        ServerListPing::Respond {
            online_players: -1,
            max_players: -1,
            description: "Hello Valence! ".into_text() + "Text Example".color(Color::AQUA),
            favicon_png: Some(include_bytes!("../assets/logo-64x64.png").as_slice().into()),
            player_sample: Default::default(),
        }
    }

    fn init(&self, server: &mut Server<Self>) {
        server.state = ServerState {
            world: create_world(server),
            blocks_stepped_on: HashSet::new(),
        };
    }

    fn update(&self, server: &mut Server<Self>) {
        
        server.clients.retain(|_, client| {
            if client.created_this_tick() {
                // Boilerplate for client initialization
                match server
                    .entities
                    .insert_with_uuid(EntityKind::Player, client.uuid(), ())
                {
                    Some((id, _)) => client.state.entity_id = id,
                    None => {
                        client.disconnect("Conflicting UUID");
                        return false;
                    }
                }

                let world_id = server.state.world;
                

                client.set_flat(true);
                client.spawn(world_id);
                client.teleport(SPAWN_POS, -90.0, 0.0);
                client.set_game_mode(GameMode::Creative);

                            }

            if client.is_disconnected() {
                server.entities.remove(client.state.entity_id);
                return false;
            }

            if client.position().y < -10.0 {
                client.teleport(SPAWN_POS, 0.0, 0.0);
            }

            let pos_under_player = BlockPos::new(
                (client.position().x -0.5).round() as i32,
                (client.position().y -0.25).floor() as i32,
                (client.position().z -0.5).round() as i32,
            );
            
            let (world_id, world) = server
                .worlds
                .iter_mut()
                .find(|w| w.0 == server.state.world)
                .unwrap();
            // run after 1 second in a new thread

            if let Some(block) = world.chunks.block_state(pos_under_player) {
                if !block.is_air() {
                   let block_stepped_on = SteppedOnBlock {
                    position: pos_under_player,
                    removal_time: SystemTime::now(),
                   };
                   server.state.blocks_stepped_on.insert(block_stepped_on);
                    
                }
            }

            let mut new_block_list: HashSet<SteppedOnBlock> = HashSet::new();
            for block in server.state.blocks_stepped_on.iter() {
                if SystemTime::now().duration_since(block.removal_time).unwrap() >= Duration::from_millis(200){
                    world.chunks.set_block_state(block.position, BlockState::AIR);
                    
                } else {
                    new_block_list.insert(block.clone());
                }
            }
            server.state.blocks_stepped_on = new_block_list;
  
            
            
            let player = server.entities.get_mut(client.state.entity_id).unwrap();

            while handle_event_default(client, player).is_some() {}

            true
        });
    }
}

// Boilerplate for creating world
fn create_world(server: &mut Server<Game>) -> WorldId {
    let dimension = server.shared.dimensions().next().unwrap();

    let (world_id, world) = server.worlds.insert(dimension.0, ());

    // Create chunks
    for z in -3..3 {
        for x in -3..3 {
            world.chunks.insert([x, z], UnloadedChunk::default(), ());
        }
    }

    // Create platform
    let platform_block = BlockState::STONE;

    for z in 0..PLATFORM_Z {
        for x in 0..PLATFORM_X {
            world
                .chunks
                .set_block_state([x, FLOOR_Y, z], platform_block);
        }
    }

    world_id
}

