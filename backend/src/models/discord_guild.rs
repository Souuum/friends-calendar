use serde::Serialize;

/// Basic public info about the Discord server this app is linked to (see
/// services::friends::get_linked_server_info). Single-server for now —
/// DISCORD_GUILD_ID is one guild, not a list.
#[derive(Debug, Clone, Serialize)]
pub struct LinkedServerInfo {
    pub id: String,
    pub name: String,
    pub icon_url: Option<String>,
    pub approximate_member_count: Option<u64>,
}

impl LinkedServerInfo {
    pub fn build_icon_url(guild_id: &str, icon_hash: Option<&str>) -> Option<String> {
        icon_hash.map(|hash| format!("https://cdn.discordapp.com/icons/{guild_id}/{hash}.png"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_icon_url_formats_the_cdn_url() {
        assert_eq!(
            LinkedServerInfo::build_icon_url("g1", Some("abc123")),
            Some("https://cdn.discordapp.com/icons/g1/abc123.png".to_string())
        );
    }

    #[test]
    fn build_icon_url_is_none_without_a_hash() {
        assert_eq!(LinkedServerInfo::build_icon_url("g1", None), None);
    }
}
