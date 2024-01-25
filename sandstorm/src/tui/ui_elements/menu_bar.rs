/*
██████████████████████████████████████████████████████████████████████████████████████████████████████████████████████████
█║ Shutdown (x) ║ Socks5 (s) ║ Sandstorm (d) ║ Users (u) ║ Auth (a) ║ 16.9KB (b) ║ Sandstorm Protocol v1       ║ 9999ms ║█
█╚══════════════╩════════════╩═══════════════╩═══════════╩══════════╩════════════╩═════════════════════════════╩════════╝█
*/

const SHUTDOWN_KEY: char = 'x';
const SOCKS5_KEY: char = 's';
const SANDSTORM_KEY: char = 'd';
const USERS_KEY: char = 'u';
const AUTH_KEY: char = 'a';
const BUFFER_KEY: char = 'b';

const SHUTDOWN_LABEL: &str = "Shutdown";
const SOCKS5_LABEL: &str = "Socks5";
const SANDSTORM_LABEL: &str = "Sandstorm";
const USERS_LABEL: &str = "Users";
const AUTH_LABEL: &str = "Auth";
const EXTRA_LABEL: &str = "Sandstorm Protocol v1";

struct MenuBar {}

impl MenuBar {
    pub fn new() -> Self {
        Self {}
    }
}
