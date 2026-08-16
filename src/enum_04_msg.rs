pub fn demo() {
  #[derive(Debug)]
  struct Text {
    text: String,
  }
  #[derive(Debug)]
  struct Image {
    src: String,
  }
  enum Msg {
    Text(Text),
    Image(Image),
  }

  let msg_list = vec![
    Msg::Text(Text {
      text: "文本消息".to_owned(),
    }),
    Msg::Image(Image {
      src: "https://www.abc.com/a.png".to_owned(),
    }),
  ];
  for msg in &msg_list {
    match msg {
      Msg::Text(text) => {
        println!("文本处理：{}", text.text);
      }
      Msg::Image(image) => {
        println!("图片处理：{}", image.src);
      }
    }
  }
}
