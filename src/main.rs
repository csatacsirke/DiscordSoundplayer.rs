//! Requires the "client", "standard_framework", and "voice" features be enabled in your
//! Cargo.toml, like so:
//!
//! ```toml
//! [dependencies.serenity]
//! git = "https://github.com/serenity-rs/serenity.git"
//! features = ["client", standard_framework", "voice"]
//! ```
use std::{borrow::Borrow, env, io::Result, path::PathBuf};

// This trait adds the `register_songbird` and `register_songbird_with` methods
// to the client builder below, making it easy to install this voice client.
// The voice client can be retrieved in any command using `songbird::get(ctx).await`.
use songbird::{SerenityInit, input};

// Import the `Context` to handle commands.
use serenity::client::Context;

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

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(deafen, join, leave, mute, play, ping, undeafen, unmute)]
struct General;

static mut global_sounds_dir: String = String::new();

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");


    unsafe {
        global_sounds_dir = env::var("SOUNDS_DIRECTORY")
            .expect("Expected a SOUNDS_DIRECTORY in the environment");    
    }
    

    let framework = StandardFramework::new()
        .configure(|c| c
                   .prefix("~"))
        .group(&GENERAL_GROUP);

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Err creating client");

    let _ = client.start().await.map_err(|why| println!("Client ended: {:?}", why));
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
        global_sounds_dir.clone()
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
            check_msg(msg.channel_id.say(&ctx.http, "Must provide a URL to a video or audio").await);

            return Ok(());
        },
    };

    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;

    let manager = songbird::get(ctx).await
        .expect("Songbird Voice client placed in at initialisation.").clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        
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

        handler.play_source(source);

        check_msg(msg.channel_id.say(&ctx.http, "Playing song").await);
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
