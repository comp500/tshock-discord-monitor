use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::model::{channel::Message, id::ChannelId, prelude::*};

use backoff::{future::FutureOperation as _, ExponentialBackoff};
use std::{collections::HashSet, env, future::Future, time::Duration};

use serde::Deserialize;

struct Handler {
	channel: ChannelId,
	http_client: reqwest::Client,
	tshock_url: reqwest::Url,
	tshock_token: String,
}

#[derive(Debug, Deserialize)]
struct WorldStatus {
	world: String,
	maxplayers: i32,
}

#[derive(Debug, Deserialize)]
struct Player {
	username: String,
}

#[derive(Debug, Deserialize)]
struct PlayerList {
	players: Vec<Player>,
}

async fn check_tshock_status(
	http_client: &reqwest::Client,
	tshock_url: &reqwest::Url,
	tshock_token: &String,
) -> Result<WorldStatus, reqwest::Error> {
	Ok(http_client
		.get(tshock_url.join("v2/server/status").unwrap())
		.query(&[("token", tshock_token.as_str())])
		.send()
		.await?
		.json::<WorldStatus>()
		.await?)
}

async fn read_tshock_player_list(
	http_client: &reqwest::Client,
	tshock_url: &reqwest::Url,
	tshock_token: &String,
) -> Result<PlayerList, reqwest::Error> {
	Ok(http_client
		.get(tshock_url.join("v2/players/list").unwrap())
		.query(&[("token", tshock_token.as_str())])
		.send()
		.await?
		.json::<PlayerList>()
		.await?)
}

#[async_trait]
impl EventHandler for Handler {
	async fn ready(&self, ctx: Context, _data_about_bot: Ready) {
		println!("Discord connection successful!");

		let tshock_url = self.tshock_url.clone();
		let http_client = self.http_client.clone();
		let tshock_token = self.tshock_token.clone();
		let channel = self.channel;

		tokio::spawn(async move {
			// Query the current max player count and world name
			println!("Attempting to connect to TShock");
			let status = (|| async {
				match check_tshock_status(&http_client, &tshock_url, &tshock_token).await {
					Ok(status) => Ok(status),
					Err(err) => {
						eprintln!("Failed to check TShock server status: {}", err);
						Err(err.into())
					}
				}
			})
			.retry(ExponentialBackoff {
				max_elapsed_time: None,
				..ExponentialBackoff::default()
			})
			.await
			.unwrap();
			let mut player_list = HashSet::new();
			let mut removed_players = vec![];

			let mut displayed_player_count = None;
			loop {
				// Query the current player list, send messages when players leave/join
				let new_player_list = (|| async {
					match read_tshock_player_list(&http_client, &tshock_url, &tshock_token).await {
						Ok(player_list) => Ok(player_list),
						Err(err) => {
							eprintln!("Failed to check TShock player list: {}", err);
							Err(err.into())
						}
					}
				})
				.retry(ExponentialBackoff {
					max_elapsed_time: None,
					..ExponentialBackoff::default()
				})
				.await
				.unwrap();

				for added_player in new_player_list
					.players
					.iter()
					.filter(|p| player_list.insert(p.username.clone()))
				{
					if let Err(err) = channel
						.say(
							&ctx,
							format!("{} joined the game", added_player.username.clone()),
						)
						.await
					{
						eprintln!("Failed to send join message: {:?}", err);
					}
				}

				player_list.retain(|player_name| {
					if !new_player_list
						.players
						.iter()
						.any(|new_p| new_p.username == *player_name)
					{
						removed_players.push(player_name.clone());
						return false;
					}
					true
				});

				for removed_player_name in removed_players.drain(..) {
					if let Err(err) = channel
						.say(&ctx, format!("{} left the game", removed_player_name))
						.await
					{
						eprintln!("Failed to send leave message: {:?}", err);
					}
				}

				if displayed_player_count == None
					|| displayed_player_count != Some(player_list.len())
				{
					displayed_player_count = Some(player_list.len());

					// Set the user activity
					ctx.set_activity(Activity::playing(
						format!(
							"Terraria: {}/{} online",
							player_list.len(),
							status.maxplayers
						)
						.as_str(),
					))
					.await;
					// Set the channel topic
					if let Err(err) = channel
						.edit(&ctx, |ch| {
							ch.topic(format!(
								"{} | Players online: {}/{}",
								status.world,
								player_list.len(),
								status.maxplayers
							));
							ch
						})
						.await
					{
						eprintln!("Failed to set channel topic: {:?}", err);
					}
				}

				// Wait 20 seconds...
				// TODO: make this configurable
				tokio::time::delay_for(Duration::from_secs(20)).await;
			}
		});
	}

	async fn message(&self, ctx: Context, msg: Message) {
		if msg.channel_id != self.channel || msg.author.bot {
			return;
		}

		// On every message, broadcast to Terraria
		let username = msg.author_nick(ctx).await.unwrap_or(msg.author.name);
		if let Err(err) = self
			.http_client
			.get(self.tshock_url.join("v2/server/broadcast").unwrap())
			.query(&[
				("token", self.tshock_token.as_str()),
				("msg", format!("({}) {}", username, msg.content).as_str()),
			])
			.send()
			.await
		{
			eprintln!("Failed to broadcast message: {:?}", err);
		}
	}
}

#[derive(Debug, serde::Deserialize)]
struct Settings {
	discord_token: String,
	tshock_token: String,
	tshock_url: String,
	discord_channel: String,
}

#[tokio::main]
async fn main() {
	let mut settings = config::Config::default();
	settings
		.merge(config::Environment::with_prefix("tdm"))
		.expect("Failed to parse environment variables");
	settings
		.merge(
			config::File::with_name(
				env::var("CONFIG_FILE")
					.unwrap_or_else(|_| "tshock_discord_monitor".into())
					.as_str(),
			)
			.required(false),
		)
		.expect("Failed to read config file");
	let settings: Settings = settings.try_into().expect("Failed to read configuration");

	println!("Connecting to Discord...");

	let mut client = Client::new(settings.discord_token)
		.event_handler(Handler {
			channel: ChannelId(
				settings
					.discord_channel
					.parse::<u64>()
					.expect("Failed to parse channel ID"),
			),
			http_client: reqwest::Client::new(),
			tshock_url: reqwest::Url::parse(settings.tshock_url.as_str())
				.expect("Failed to TShock URL"),
			tshock_token: settings.tshock_token,
		})
		.await
		.expect("Error creating client");

	if let Err(why) = client.start().await {
		println!("An error occurred while running the client: {:?}", why);
	}
}
