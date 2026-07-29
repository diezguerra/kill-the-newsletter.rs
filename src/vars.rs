use std::env::VarError;

pub fn web_url() -> String {
    required_var("WEB_URL")
}

pub fn email_domain() -> String {
    required_var("EMAIL_DOMAIN")
}

pub fn static_folder() -> String {
    required_var("STATIC_FOLDER")
}

pub fn database_url() -> String {
    required_var("DATABASE_URL")
}

fn required_var(name: &str) -> String {
    match std::env::var(name) {
        Ok(v) => v,
        Err(VarError::NotPresent) => {
            panic!("Required environment variable {} is not set", name)
        }
        Err(VarError::NotUnicode(_)) => {
            panic!("Environment variable {} contains invalid UTF-8", name)
        }
    }
}
