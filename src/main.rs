use std::collections::HashMap;

use tgbot::{
    async_trait,
    longpoll::LongPoll,
    methods::{DeleteMessage, KickChatMember, RestrictChatMember, SendMessage, UnbanChatMember},
    types::{
        InlineKeyboardButton, Integer, Message, MessageData::NewChatMembers, ParseMode::Markdown,
        Update, UpdateKind, User,
    },
    Api, UpdateHandler,
};
use tokio::time::Duration;

type UserId = Integer;
type ChatId = Integer;
type MessageId = Integer;

#[tokio::main]
async fn main() {
    let token = std::env::var("TOKEN").expect("TOKEN env var isn't set");
    let client = Api::new(token).expect("Failed to create API client");
    let bot = Bot::new(client.clone());
    LongPoll::new(client, bot).run().await;
}

struct Bot {
    client: Api,
    pending_approvals: HashMap<(ChatId, UserId), MessageId>,
}

impl Bot {
    fn new(client: Api) -> Self {
        Self {
            client,
            pending_approvals: Default::default(),
        }
    }

    async fn welcome(&mut self, user: &User, join_msg: &Message) {
        let chat_id = join_msg.get_chat_id();
        self.silence(chat_id, user).await;

        // TODO: allow text to be specified externally
        let text = format!(
            "Olá, {}! Seja bem vinda(o) à comunidade de Rust do Brasil! \n\n\

            Já programa em Rust? Separamos na [mensagem fixada](https://t.me/rustlangbr/168181) \
            um material para ajudar quem está iniciando na linguagem, confere lá! 🦀 \n\n\

            Ah, e para provar que você é uma pessoa de verdade e está ciente do \
            [nosso código de conduta](https://www.rust-lang.org/pt-BR/policies/code-of-conduct), \
            pressione o botão abaixo, por favor!",
            user.first_name
        );

        let button = InlineKeyboardButton::with_callback_data(
            "Sou humana(o) e estou ciente do código de conduta.",
            user.id.to_string(), // this isn't used, but the 'data' field can't be empty
        );

        let prepared_msg = SendMessage::new(chat_id, text)
            .reply_to_message_id(join_msg.id)
            .parse_mode(Markdown)
            .disable_web_page_preview(true)
            .reply_markup(vec![vec![button]]);

        if let Ok(msg) = self.client.execute(prepared_msg).await {
            self.pending_approvals.insert((chat_id, user.id), msg.id);

            // TODO: parameterize this timeout as well
            tokio::time::delay_for(Duration::from_secs(30)).await;

            if self.pending_approvals.contains_key(&(chat_id, user.id)) {
                let _ = self
                    .client
                    .execute(KickChatMember::new(chat_id, user.id))
                    .await;

                self.clear_pending_approval(chat_id, user.id).await;

                let _ = self
                    .client
                    .execute(UnbanChatMember::new(chat_id, user.id))
                    .await;
            }
        }
    }

    async fn approve_user(&mut self, chat_id: ChatId, user: &User) {
        self.unsilence(chat_id, user).await;
        self.clear_pending_approval(chat_id, user.id).await;
    }

    async fn clear_pending_approval(&mut self, chat_id: ChatId, user_id: UserId) {
        if let Some(msg_id) = self.pending_approvals.remove(&(chat_id, user_id)) {
            let _ = self
                .client
                .execute(DeleteMessage::new(chat_id, msg_id))
                .await;
        }
    }
    async fn silence(&self, chat_id: ChatId, user: &User) {
        let _ = self
            .client
            .execute(RestrictChatMember::new(chat_id, user.id).restrict_all())
            .await;
    }

    async fn unsilence(&self, chat_id: ChatId, user: &User) {
        let _ = self
            .client
            .execute(RestrictChatMember::new(chat_id, user.id).allow_all())
            .await;
    }
}

#[async_trait]
impl UpdateHandler for Bot {
    async fn handle(&mut self, update: Update) {
        match update.kind {
            UpdateKind::Message(msg) => {
                if let NewChatMembers(users) = &msg.data {
                    for user in users {
                        self.welcome(&user, &msg).await;
                    }
                }
            }

            UpdateKind::CallbackQuery(callback) => {
                if let Some(msg) = &callback.message {
                    self.approve_user(msg.get_chat_id(), &callback.from).await;
                }
            }

            _ => {}
        }
    }
}
