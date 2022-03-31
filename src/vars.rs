use dotenv_codegen::dotenv;

pub const WEB_URL: &str = dotenv!("WEB_URL");
pub const EMAIL_DOMAIN: &str = dotenv!("EMAIL_DOMAIN");
pub const DATABASE_URL: &str = dotenv!("DATABASE_URL");
pub const STATIC_FOLDER: &str = dotenv!("STATIC_FOLDER");
