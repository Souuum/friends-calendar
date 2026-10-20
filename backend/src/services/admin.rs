//! Who may change the bot's configuration.
//!
//! An admin is whoever Discord already says runs the server: its owner, or
//! anyone whose roles grant Administrator or Manage Server. Nothing is
//! stored here - roles change on Discord, and a copy kept in this database
//! would go stale the moment someone is demoted. The cost is two bot-token
//! calls per check, which only happens on the handful of config endpoints
//! and once when the app loads.
//!
//! Same `base_url`-as-parameter shape as every other Discord call, so it is
//! wiremock-testable.

use anyhow::{Result, anyhow};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

/// Discord permission bits that make someone an admin here.
/// <https://discord.com/developers/docs/topics/permissions>
pub const ADMINISTRATOR: u64 = 1 << 3;
pub const MANAGE_GUILD: u64 = 1 << 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminCheck {
    Admin,
    /// Not an admin, including not being a member of the server at all.
    NotAdmin,
    /// The bot can't see the server, so nobody's permissions can be read.
    BotNotInGuild,
}

#[derive(Deserialize)]
struct GuildRole {
    id: String,
    /// A decimal string: the bitfield outgrew what JSON numbers hold exactly.
    permissions: String,
}

#[derive(Deserialize)]
struct GuildPayload {
    owner_id: String,
    roles: Vec<GuildRole>,
}

#[derive(Deserialize)]
struct MemberPayload {
    roles: Vec<String>,
}

/// The decision itself, with no I/O.
///
/// Permissions are the union of the `@everyone` role - whose id *is* the
/// guild id - and every role the member holds. Channel overwrites don't
/// matter: Manage Server and Administrator are server-wide.
pub fn grants_admin(
    guild_id: &str,
    owner_id: &str,
    user_id: &str,
    roles: &[(String, u64)],
    member_role_ids: &[String],
) -> bool {
    if user_id == owner_id {
        return true;
    }
    let permissions = roles
        .iter()
        .filter(|(id, _)| id == guild_id || member_role_ids.contains(id))
        .fold(0u64, |acc, (_, bits)| acc | bits);
    permissions & (ADMINISTRATOR | MANAGE_GUILD) != 0
}

/// Both ids are interpolated into request paths, so anything that isn't a
/// plain path segment is refused before it reaches Discord.
fn is_path_safe(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub async fn check_guild_admin(
    base_url: &str,
    http: &Client,
    bot_token: &str,
    guild_id: &str,
    user_id: &str,
) -> Result<AdminCheck> {
    if !is_path_safe(guild_id) || !is_path_safe(user_id) {
        return Ok(AdminCheck::NotAdmin);
    }
    let auth = format!("Bot {bot_token}");

    let response = http
        .get(format!("{base_url}/guilds/{guild_id}"))
        .header("Authorization", &auth)
        .send()
        .await?;
    // Discord answers 404 for a guild the bot isn't in, not 403.
    if response.status() == StatusCode::NOT_FOUND || response.status() == StatusCode::FORBIDDEN {
        return Ok(AdminCheck::BotNotInGuild);
    }
    if !response.status().is_success() {
        return Err(anyhow!("Discord API error ({})", response.status()));
    }
    let guild: GuildPayload = response.json().await?;

    if user_id == guild.owner_id {
        return Ok(AdminCheck::Admin);
    }

    let response = http
        .get(format!("{base_url}/guilds/{guild_id}/members/{user_id}"))
        .header("Authorization", &auth)
        .send()
        .await?;
    // Not a member of the server: certainly not one of its admins.
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(AdminCheck::NotAdmin);
    }
    if !response.status().is_success() {
        return Err(anyhow!("Discord API error ({})", response.status()));
    }
    let member: MemberPayload = response.json().await?;

    // An unparseable bitfield grants nothing rather than failing the check.
    let roles: Vec<(String, u64)> = guild
        .roles
        .into_iter()
        .map(|r| (r.id, r.permissions.parse().unwrap_or(0)))
        .collect();

    Ok(
        if grants_admin(guild_id, &guild.owner_id, user_id, &roles, &member.roles) {
            AdminCheck::Admin
        } else {
            AdminCheck::NotAdmin
        },
    )
}

/// Mocks for any test that exercises an admin-gated endpoint.
#[cfg(test)]
pub mod test_support {
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Mounts `GET /guilds/{guild_id}` and `GET /guilds/{guild_id}/members/{user_id}`,
    /// with `user_id` holding a role that grants Manage Server when `is_admin`.
    ///
    /// The guild payload also carries `id`/`name`/`icon`, so the same mock
    /// serves `services::guilds::register_guild_by_id`, which reads them.
    pub async fn mount_guild_member(
        server: &MockServer,
        guild_id: &str,
        user_id: &str,
        is_admin: bool,
    ) {
        Mock::given(method("GET"))
            .and(path(format!("/guilds/{guild_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": guild_id,
                "name": "The Hangout",
                "icon": null,
                "owner_id": "someone-else",
                "roles": [
                    { "id": guild_id, "permissions": "0" },
                    { "id": "role-manager", "permissions": "32" }
                ]
            })))
            .mount(server)
            .await;

        let roles = if is_admin {
            json!(["role-manager"])
        } else {
            json!([])
        };
        Mock::given(method("GET"))
            .and(path(format!("/guilds/{guild_id}/members/{user_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "roles": roles })))
            .mount(server)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn roles(list: &[(&str, u64)]) -> Vec<(String, u64)> {
        list.iter().map(|(id, p)| (id.to_string(), *p)).collect()
    }

    #[test]
    fn the_owner_is_an_admin_without_any_role() {
        assert!(grants_admin("g", "owner", "owner", &[], &[]));
    }

    #[test]
    fn manage_server_or_administrator_through_a_role_grants_admin() {
        let r = roles(&[("g", 0), ("mod", MANAGE_GUILD), ("boss", ADMINISTRATOR)]);
        assert!(grants_admin("g", "o", "u", &r, &["mod".to_string()]));
        assert!(grants_admin("g", "o", "u", &r, &["boss".to_string()]));
    }

    #[test]
    fn everyone_role_permissions_count_for_every_member() {
        // @everyone's id is the guild id; a member holds it implicitly.
        let r = roles(&[("g", MANAGE_GUILD)]);
        assert!(grants_admin("g", "o", "u", &r, &[]));
    }

    #[test]
    fn other_permissions_and_roles_the_member_lacks_grant_nothing() {
        // Send Messages + Manage Channels, and an admin role they don't hold.
        let r = roles(&[("g", 2048), ("helper", 16), ("boss", ADMINISTRATOR)]);
        assert!(!grants_admin("g", "o", "u", &r, &["helper".to_string()]));
    }

    #[tokio::test]
    async fn reads_roles_from_discord() {
        let server = MockServer::start().await;
        test_support::mount_guild_member(&server, "111", "222", true).await;
        let http = Client::new();
        let check = check_guild_admin(&server.uri(), &http, "t", "111", "222")
            .await
            .unwrap();
        assert_eq!(check, AdminCheck::Admin);

        let other = MockServer::start().await;
        test_support::mount_guild_member(&other, "111", "222", false).await;
        let check = check_guild_admin(&other.uri(), &http, "t", "111", "222")
            .await
            .unwrap();
        assert_eq!(check, AdminCheck::NotAdmin);
    }

    #[tokio::test]
    async fn someone_outside_the_server_is_not_an_admin() {
        let server = MockServer::start().await;
        test_support::mount_guild_member(&server, "111", "222", true).await;
        // No member mock for "333": wiremock answers 404, as Discord does.
        let check = check_guild_admin(&server.uri(), &Client::new(), "t", "111", "333")
            .await
            .unwrap();
        assert_eq!(check, AdminCheck::NotAdmin);
    }

    #[tokio::test]
    async fn a_server_the_bot_cannot_see_is_reported_as_such() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/guilds/111"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({ "code": 10004 })))
            .mount(&server)
            .await;
        let check = check_guild_admin(&server.uri(), &Client::new(), "t", "111", "222")
            .await
            .unwrap();
        assert_eq!(check, AdminCheck::BotNotInGuild);
    }

    #[tokio::test]
    async fn ids_that_are_not_plain_path_segments_never_reach_discord() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        let http = Client::new();
        for (guild, user) in [("111/../x", "222"), ("111", "222?x=1"), ("", "222")] {
            let check = check_guild_admin(&server.uri(), &http, "t", guild, user)
                .await
                .unwrap();
            assert_eq!(check, AdminCheck::NotAdmin);
        }
    }
}
