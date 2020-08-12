use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::model::{id::ChannelId, channel::Message, prelude::*};

use std::{time::Duration, env, collections::HashSet};
use tokio::time::interval;
use serde::Deserialize;

struct Handler {
	channel: ChannelId,
	http_client: reqwest::Client,
	tshock_url: reqwest::Url,
	tshock_token: String
}

#[derive(Debug, Deserialize)]
struct WorldStatus {
	world: String,
	maxplayers: i32
}

#[derive(Debug, Deserialize)]
struct Player {
	username: String
}

#[async_trait]
impl EventHandler for Handler {
	async fn ready(&self, ctx: Context, _data_about_bot: Ready) {
		let tshock_url = self.tshock_url.clone();
		let http_client = self.http_client.clone();
		let tshock_token = self.tshock_token.clone();
		let channel = self.channel.clone();

		tokio::spawn(async move {
			// Query the current max player count and world name
			// TODO: better error handling
			let status = http_client.get(tshock_url.join("v2/server/status").unwrap())
				.query(&[("token", tshock_token.as_str())])
				.send().await.unwrap().json::<WorldStatus>().await.unwrap();
			let mut player_list = HashSet::new();
			let mut removed_players = vec![];

			// Every 30 seconds...
			let mut interval = tokio::time::interval(Duration::from_secs(30));
			loop {
				// Query the current player list, send messages when players leave/join
				// TODO: better error handling
				let new_player_list = http_client.get(tshock_url.join("v2/server/status").unwrap())
					.query(&[("token", tshock_token.as_str())])
					.send().await.unwrap().json::<Vec<Player>>().await.unwrap();
				
				let mut list_changed = false;
				
				for added_player in new_player_list.iter().filter(|p| player_list.insert(p.username.clone())) {
					// TODO: better error handling
					channel.say(&ctx, format!("{} joined the game", added_player.username.clone())).await.unwrap();
					list_changed = true;
				}

				player_list.retain(|player_name| {
					if !new_player_list.iter().any(|new_p| new_p.username == *player_name) {
						removed_players.push(player_name.clone());
						return false;
					}
					true
				});

				for removed_player_name in removed_players.drain(..) {
					// TODO: better error handling
					channel.say(&ctx, format!("{} left the game", removed_player_name)).await.unwrap();
					list_changed = true;
				}

				if list_changed {
					// Set the user activity
					ctx.set_activity(Activity::playing(format!("Terraria {}/{}", player_list.len(), status.maxplayers).as_str())).await;
					// Set the channel topic
					channel.edit(&ctx, |ch| {
						ch.topic(format!("{} | Players online: {}/{}", status.world, player_list.len(), status.maxplayers));
						ch
					}).await.unwrap();
				}
				
				interval.tick().await;
			}
		});
	}

	async fn message(&self, ctx: Context, msg: Message) {
		// On every message, broadcast to Terraria
		if msg.channel_id == self.channel {
			let username = msg.author_nick(ctx).await.unwrap_or(msg.author.name);
			self.http_client.get(self.tshock_url.join("v2/server/broadcast").unwrap())
				.query(&[("token", self.tshock_token.as_str()), ("msg", format!("({}) {}", username, msg.content).as_str())])
				.send().await.unwrap();
			// TODO: handle error
		}
	}
}

#[derive(Debug, serde::Deserialize)]
struct Settings {
	discord_token: String,
	tshock_token: String,
	tshock_url: String,
	discord_channel: String
}

#[tokio::main]
async fn main() {
	let mut settings = config::Config::default();
	settings.merge(config::Environment::with_prefix("tdm")).unwrap();
	settings.merge(config::File::with_name(
		env::var("CONFIG_FILE").unwrap_or("tshock_discord_monitor".into()).as_str()).required(false)).unwrap();
	println!("{:?}", settings);
	let settings: Settings = settings.try_into().unwrap();

	let mut client = Client::new(settings.discord_token)
		.event_handler(Handler {
			channel: ChannelId(settings.discord_channel.parse::<u64>().unwrap()),
			http_client: reqwest::Client::new(),
			tshock_url: reqwest::Url::parse(settings.tshock_url.as_str()).unwrap(),
			tshock_token: settings.tshock_token
		})
		.await
		.expect("Error creating client");

	if let Err(why) = client.start().await {
		println!("An error occurred while running the client: {:?}", why);
	}
}