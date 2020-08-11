use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::model::{id::ChannelId, channel::Message};

use std::env;


struct Handler {
	channel: ChannelId,
	http_client: reqwest::Client,
	tshock_url: String,
	tshock_token: String
}

#[async_trait]
impl EventHandler for Handler {
	async fn message(&self, _ctx: Context, msg: Message) {
		if msg.channel_id == self.channel {
			self.http_client.get(self.tshock_url.as_str())
				.query(&["token", self.tshock_token.as_str()])
				.query(&["msg", msg.content.as_str()])
				.send().await;
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
			tshock_url: settings.tshock_url,
			tshock_token: settings.tshock_token
		})
		.await
		.expect("Error creating client");

	if let Err(why) = client.start().await {
		println!("An error occurred while running the client: {:?}", why);
	}
}