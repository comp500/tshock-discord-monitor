# tshock-discord-monitor
A discord bot that uses the TShock REST API to show player count and forward messages Discord -> Terraria.

## Setup
1. Enable the TShock REST API as shown in https://tshock.readme.io/reference#rest-api-endpoints and get a token (the easiest way is to make a random application REST token)
2. Create an application at https://discord.com/developers/applications/
3. Create a Bot user for the application, then copy the Discord token for that bot
4. Download the tshock-discord-monitor executable for your system from the releases page
5. Create a file in the same folder as the executable called `tshock-discord-monitor.toml` with the following contents:

```toml
discord_token = "The Discord token you copied earlier"
tshock_token = "Your TShock REST API token"
tshock_url = "http://localhost:7878/"
discord_channel = "Your Discord channel's ID"
```

Then you can simply run the tshock-discord-monitor executable to start the bot! To get the channel ID for the channel you would like to use, see https://support.discord.com/hc/en-us/articles/206346498-Where-can-I-find-my-User-Server-Message-ID-.

If the TShock server isn't running on the same system as the system running the bot, replace `localhost` with the IP of that system.