use super::login::{AccountImpl, Account, AccountType};


pub struct MicrosoftAccount {}

impl MicrosoftAccount {
    const CLIENT_ID: &'static str = env!("CLIENT_ID");
    const CLIENT_SECRET: &'static str = env!("CLIENT_SECRET");
    const REDIRECT_URI: &'static str = env!("REDIRECT_URI");

    fn create_user_url() -> String {
        format!("https://login.live.com/oauth20_authorize.srf\
?client_id={}\
&response_type=code\
&redirect_uri={}\
&scope=XboxLive.signin%20offline_access", Self::CLIENT_ID, Self::REDIRECT_URI)
    }
}

impl AccountImpl for MicrosoftAccount {
    fn login(&self, username: &str, password: &str, token: &str) -> Result<super::login::Account, super::Error> {
        unimplemented!()
    }

    fn join_server(
        &self,
        account: &super::login::Account,
        server_id: &str,
        shared_key: &[u8],
        public_key: &[u8],
    ) -> Result<(), super::Error> {
        todo!()
    }

    fn refresh(&self, account: super::login::Account, token: &str) -> Result<super::login::Account, super::Error> {
        todo!()
    }

    fn append_head_img_data(&self, account: &mut super::login::Account) -> Result<(), super::Error> {
        todo!()
    }

    fn custom_auth(&self) -> Result<super::login::Account, super::Error> {
        let user_url = Self::create_user_url();

        Ok(
            Account {
                name: todo!(),
                uuid: todo!(),
                verification_tokens: todo!(),
                head_img_data: todo!(),
                account_type: AccountType::Microsoft,
            }
        )
    }
}
