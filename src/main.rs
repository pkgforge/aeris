use iced::{
    Alignment,
    widget::{button, column, row, text},
};

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title(App::title)
        .run()
}

#[derive(Default)]
struct App {
    value: u32,
}

impl App {
    fn title(&self) -> String {
        String::from("Aeris")
    }

    fn update(&mut self, event: Message) -> iced::Task<Message> {
        match event {
            Message::Increment => self.value += 1,
            Message::Decrement => {
                if self.value > 0 {
                    self.value -= 1;
                }
            }
        }
        iced::Task::none()
    }

    fn view(&self) -> iced::Element<'_, Message> {
        column![
            text(format!("Count: {}", self.value)).size(24),
            row![
                button("+")
                    .on_press(Message::Increment)
                    .padding(10)
                    .width(30),
                button("-")
                    .on_press(Message::Decrement)
                    .padding(10)
                    .width(30)
            ]
            .spacing(10)
        ]
        .spacing(20)
        .padding(40)
        .into()
    }
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Increment,
    Decrement,
}
