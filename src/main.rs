//! Requires the "client", "standard_framework", and "voice" features be enabled in your
//! Cargo.toml, like so:
//!
//! ```toml
//! [dependencies.serenity]
//! git = "https://github.com/serenity-rs/serenity.git"
//! features = ["client", standard_framework", "voice"]
//! ```
use std::{env, sync::Arc};

// This trait adds the `register_songbird` and `register_songbird_with` methods
// to the client builder below, making it easy to install this voice client.
// The voice client can be retrieved in any command using `songbird::get(ctx).await`.
use songbird::{Call, SerenityInit};

// Import the `Context` to handle commands.
use serenity::{client::{Context, bridge::gateway::ShardManager}, model::guild::GuildStatus, prelude::Mutex};

use serenity::{
	async_trait,
	client::{Client, EventHandler},
	framework::{
		StandardFramework,
		standard::{
			Args, CommandResult,
			macros::{command, group},
		},
	},
	model::{channel::Message, gateway::Ready},
	Result as SerenityResult,
};

struct HandlerState {
	ctx: Option<Context>,
	guilds: Vec<GuildStatus>,
}


impl HandlerState {
	fn new() -> HandlerState {
		HandlerState {
			ctx: None,
			guilds: Vec::new(),
		}
	}
}

struct Handler {
	state: Arc<Mutex<HandlerState>>,
}

impl Handler {
	fn new() -> Handler {
		return Handler {
			state: Arc::new( Mutex::new(HandlerState::new())),
		}
	}
}

#[async_trait]
impl EventHandler for Handler {
	async fn ready(&self, ctx: Context, ready: Ready) {
		println!("{} is connected!", ready.user.name);
		let mut state = self.state.lock().await;
		state.guilds = ready.guilds.clone();
		state.ctx = Some(ctx);
	}
}

#[group]
#[commands(deafen, join, leave, mute, play, ping, undeafen, unmute)]
struct General;

static mut GLOBA_SOUNDS_DIR: String = String::new();

async fn find_active_voice_channel(state: &HandlerState) -> Result<Arc<Mutex<Call>>, String> {

	let guilds = &state.guilds;
	let ctx = match state.ctx.as_ref() {
		Some(ctx) => ctx,
		_ => return Err("State is uninitialized".to_string()),
	};

	for guild in guilds {
		let guild_id = match guild {
			GuildStatus::OnlinePartialGuild(_) => guild.id(),
			GuildStatus::OnlineGuild(guild) => guild.id,
			GuildStatus::Offline(guild) => guild.id,
			_ => panic!(),
		};

		let manager = songbird::get(ctx).await
			.expect("Songbird Voice client placed in at initialisation.").clone();

		if let Some(voice_channel) = manager.get(guild_id) {
			return Ok(voice_channel);
		}
	};

	return Err(String::from("No active voice channel found"));
}

async fn process_input(input: &str, state: Arc<Mutex<HandlerState>>) -> Result<String, String> {
	let state = state.lock().await;
	

	let path = match find_path_for_name(&input) {
		Some(path) => path,
		_ => {
			return Err("no matching file found".to_string());
		}
	};

	let file_name = match path.file_name().and_then(|file_name| file_name.to_str() ) {
		Some(file_name) => file_name.to_string(),
		_ => {
			return Err("Invalid path".to_string());
		}
	};
	

	let source = match songbird::input::ffmpeg(path).await {
		Ok(source) => source,
		_ => {
			return Err(std::format!("Invalid file: {}", file_name).to_string());
		},
	};

	let voice_channel = find_active_voice_channel(&state).await?;

	let mut voice_channel = voice_channel.lock().await;
	voice_channel.play_source(source);

	return Ok(std::format!("Playing {}", file_name));
}

async fn command_line_loop(state: Arc<Mutex<HandlerState>>, shard_manager: Arc<Mutex<ShardManager>>) {
	
	println!("Starting command line interface");

	let stdin = async_std::io::stdin();
	
	loop { 
		let mut line = String::new();
		let result = stdin.read_line(&mut line).await;

		if result.is_ok() {
			match line.as_str().trim() {
				"exit" => { 
					shard_manager.lock().await.shutdown_all().await;
					break;
				},
				line => { 
					match process_input(line, state.clone()).await {
						Ok(msg) => { println!("{}", msg) },
						Err(msg) => { println!("{}", msg) },
					}
				},
			}
		}
	}
	


}

#[command]
#[only_in(guilds)]
async fn deafen(ctx: &Context, msg: &Message) -> CommandResult {
	let guild = msg.guild(&ctx.cache).await.unwrap();
	let guild_id = guild.id;

	let manager = songbird::get(ctx).await
		.expect("Songbird Voice client placed in at initialisation.").clone();

	let handler_lock = match manager.get(guild_id) {
		Some(handler) => handler,
		None => {
			check_msg(msg.reply(ctx, "Not in a voice channel").await);

			return Ok(());
		},
	};

	let mut handler = handler_lock.lock().await;

	if handler.is_deaf() {
		check_msg(msg.channel_id.say(&ctx.http, "Already deafened").await);
	} else {
		if let Err(e) = handler.deafen(true).await {
			check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
		}

		check_msg(msg.channel_id.say(&ctx.http, "Deafened").await);
	}

	Ok(())
}

#[command]
#[only_in(guilds)]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
	let guild = msg.guild(&ctx.cache).await.unwrap();
	let guild_id = guild.id;

	let channel_id = guild
		.voice_states.get(&msg.author.id)
		.and_then(|voice_state| voice_state.channel_id);

	let connect_to = match channel_id {
		Some(channel) => channel,
		None => {
			check_msg(msg.reply(ctx, "Not in a voice channel").await);

			return Ok(());
		}
	};

	let manager = songbird::get(ctx).await
		.expect("Songbird Voice client placed in at initialisation.").clone();

	let _handler = manager.join(guild_id, connect_to).await;

	Ok(())
}

#[command]
#[only_in(guilds)]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
	let guild = msg.guild(&ctx.cache).await.unwrap();
	let guild_id = guild.id;

	let manager = songbird::get(ctx).await
		.expect("Songbird Voice client placed in at initialisation.").clone();
	let has_handler = manager.get(guild_id).is_some();

	if has_handler {
		if let Err(e) = manager.remove(guild_id).await {
			check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
		}

		check_msg(msg.channel_id.say(&ctx.http, "Left voice channel").await);
	} else {
		check_msg(msg.reply(ctx, "Not in a voice channel").await);
	}

	Ok(())
}

#[command]
#[only_in(guilds)]
async fn mute(ctx: &Context, msg: &Message) -> CommandResult {
	let guild = msg.guild(&ctx.cache).await.unwrap();
	let guild_id = guild.id;

	let manager = songbird::get(ctx).await
		.expect("Songbird Voice client placed in at initialisation.").clone();

	let handler_lock = match manager.get(guild_id) {
		Some(handler) => handler,
		None => {
			check_msg(msg.reply(ctx, "Not in a voice channel").await);

			return Ok(());
		},
	};

	let mut handler = handler_lock.lock().await;

	if handler.is_mute() {
		check_msg(msg.channel_id.say(&ctx.http, "Already muted").await);
	} else {
		if let Err(e) = handler.mute(true).await {
			check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
		}

		check_msg(msg.channel_id.say(&ctx.http, "Now muted").await);
	}

	Ok(())
}

#[command]
async fn ping(context: &Context, msg: &Message) -> CommandResult {
	check_msg(msg.channel_id.say(&context.http, "Pong!").await);

	Ok(())
}

// #[command]
// #[only_in(guilds)]
// async fn play(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
//     let url = match args.single::<String>() {
//         Ok(url) => url,
//         Err(_) => {
//             check_msg(msg.channel_id.say(&ctx.http, "Must provide a URL to a video or audio").await);

//             return Ok(());
//         },
//     };

//     if !url.starts_with("http") {
//         check_msg(msg.channel_id.say(&ctx.http, "Must provide a valid URL").await);

//         return Ok(());
//     }

//     let guild = msg.guild(&ctx.cache).await.unwrap();
//     let guild_id = guild.id;

//     let manager = songbird::get(ctx).await
//         .expect("Songbird Voice client placed in at initialisation.").clone();

//     if let Some(handler_lock) = manager.get(guild_id) {
//         let mut handler = handler_lock.lock().await;
		

//         let source = match songbird::ytdl(&url).await {
//             Ok(source) => source,
//             Err(why) => {
//                 println!("Err starting source: {:?}", why);

//                 check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);

//                 return Ok(());
//             },
//         };

//         handler.play_source(source);

//         check_msg(msg.channel_id.say(&ctx.http, "Playing song").await);
//     } else {
//         check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel to play in").await);
//     }

//     Ok(())
// }

fn soundboard_sanitize(str: &str) -> String {
	let str = String::from(str);
	let str = str.to_lowercase();
	let str = str.replace("ű", "u");
	let str = str.replace("ü", "u");
	let str = str.replace("ú", "u");
	let str = str.replace("ö", "o");
	let str = str.replace("ő", "o");
	let str = str.replace("ó", "o");
	let str = str.replace("á", "a");
	let str = str.replace("í", "i");
	let str = str.replace("é", "e");
	let str = str.replace(" ", "");
	return str;
}

fn soundboard_compare(file_name: &str, name_chunk: &str) -> bool {
	let file_name = soundboard_sanitize(file_name);
	let name_chunk = soundboard_sanitize(name_chunk);

	return file_name.starts_with(&name_chunk);
}


fn find_path_for_name(name: &str) -> Option<std::path::PathBuf> {
	let mut files = Vec::<std::path::PathBuf>::new();

	let sounds_dir = unsafe {
		GLOBA_SOUNDS_DIR.clone()
	};

	for entry in std::fs::read_dir(sounds_dir).ok()? {
		let entry = entry.ok()?;

		let path = entry.path();
		if path.is_dir() {
			// visit_dirs(&path, cb)?;
		} else {
			files.push(path.clone());
		}
	}

	let extract_name = |path: &std::path::PathBuf| -> String {
		path
			.file_name()
			.and_then(|path| path.to_str())
			.unwrap_or("NOFILENAMEERROR")
			.to_string()
	};

	let path = files
		.iter()
		.filter(|&path| soundboard_compare(&extract_name(path), name))
		.next()?;
		//.and_then(|path| Some(path.clone()))?;

	
	return Some(path.clone());
}

#[command]
#[only_in(guilds)]
async fn play(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {

	let file_name_chunk = match args.single::<String>() {
		Ok(file_name_chunk) => file_name_chunk,
		Err(_) => {
			check_msg(msg.channel_id.say(&ctx.http, "No matching sound.").await);

			return Ok(());
		},
	};

	
	let path = match find_path_for_name(&file_name_chunk) {
		Some(path) => path,
		_ => {
			check_msg(msg.channel_id.say(&ctx.http, "no matching file found").await);
			return Ok(());
		}
	};

	let source = match songbird::input::ffmpeg(path).await {
		Ok(source) => source,
		_ => {
			check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);
			return Ok(());
		},
	};



	let guild = msg.guild(&ctx.cache).await.unwrap();
	let guild_id = guild.id;

	let manager = songbird::get(ctx).await
		.expect("Songbird Voice client placed in at initialisation.").clone();

	if let Some(handler_lock) = manager.get(guild_id) {
		let mut handler = handler_lock.lock().await;
		
		handler.play_source(source);

		let reply_msg = std::format!("Playing song {}", 2);
		check_msg(msg.channel_id.say(&ctx.http, reply_msg).await);
	} else {
		check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel to play in").await);
	}

	Ok(())
}

#[command]
#[only_in(guilds)]
async fn undeafen(ctx: &Context, msg: &Message) -> CommandResult {
	let guild = msg.guild(&ctx.cache).await.unwrap();
	let guild_id = guild.id;

	let manager = songbird::get(ctx).await
		.expect("Songbird Voice client placed in at initialisation.").clone();

	if let Some(handler_lock) = manager.get(guild_id) {
		let mut handler = handler_lock.lock().await;
		if let Err(e) = handler.deafen(false).await {
			check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
		}

		check_msg(msg.channel_id.say(&ctx.http, "Undeafened").await);
	} else {
		check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel to undeafen in").await);
	}

	Ok(())
}

#[command]
#[only_in(guilds)]
async fn unmute(ctx: &Context, msg: &Message) -> CommandResult {
	let guild = msg.guild(&ctx.cache).await.unwrap();
	let guild_id = guild.id;
	
	let manager = songbird::get(ctx).await
		.expect("Songbird Voice client placed in at initialisation.").clone();

	if let Some(handler_lock) = manager.get(guild_id) {
		let mut handler = handler_lock.lock().await;
		if let Err(e) = handler.mute(false).await {
			check_msg(msg.channel_id.say(&ctx.http, format!("Failed: {:?}", e)).await);
		}

		check_msg(msg.channel_id.say(&ctx.http, "Unmuted").await);
	} else {
		check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel to unmute in").await);
	}

	Ok(())
}

/// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
	if let Err(why) = result {
		println!("Error sending message: {:?}", why);
	}
}


#[tokio::main]
async fn main() {
	tracing_subscriber::fmt::init();
	
	// Configure the client with your Discord bot token in the environment.
	let token = env::var("DISCORD_TOKEN")
		.expect("Expected (DISCORD_TOKEN) in the environment");


	unsafe {
		GLOBA_SOUNDS_DIR = env::var("SOUNDS_DIRECTORY")
			.expect("Expected (SOUNDS_DIRECTORY) in the environment");    
	}
	

	let framework = StandardFramework::new()
		.configure(|c| c
				   .prefix("~"))
		.group(&GENERAL_GROUP);

	let handler = Handler::new();

	let state = handler.state.clone();

	let mut client = Client::builder(&token)
		.event_handler(handler)
		.framework(framework)
		.register_songbird()
		.await
		.expect("Err creating client");


	let shard_manager = client.shard_manager.clone();

	let client_task = client.start();
	let command_line_task = command_line_loop(state, shard_manager);


	let (cllient_result, _) = futures::join!(client_task, command_line_task);

	let _ = cllient_result.map_err(|why| println!("Client ended: {:?}", why));

	//let _ = client_task.await.map_err(|why| println!("Client ended: {:?}", why));
	
}
