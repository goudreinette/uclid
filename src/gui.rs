/// An example of a chat web application server
use ws::{listen, Handler, Message, Request, Response, Result, Sender};


struct BrowserTabEditor {

}

impl BrowserTabEditor {

}

impl Editor for BrowserTabEditor {
    fn size(&self) -> (i32, i32) {
        (0,0)
    }

    fn position(&self) -> (i32, i32) {
        (0,0)
    }

    fn open(&mut self, window: *mut c_void) -> bool {
        thread::spawn(|| {
            if webbrowser::open("http://localhost:8000").is_ok() {
                // ...
            }

            listen("127.0.0.1:8000", |out| Server { out }).unwrap();
        });

        self.close();

        true
    }

    fn is_open(&mut self) -> bool {
        false
    }
}


// fn get_editor(&mut self) -> Option<Box<dyn Editor>> {
//     let gui = vst_gui::new_plugin_gui(
//         String::from(""),
//         create_javascript_callback(0),
//         None);

//     thread::spawn(|| {
//         if webbrowser::open("http://localhost:8000").is_ok() {
//             // ...
//         }

//         listen("127.0.0.1:8000", |out| Server { out }).unwrap();
//     });
    
//     Some(Box::new(gui))
// }


fn create_javascript_callback(n: i32) -> vst_gui::JavascriptCallback
{
    Box::new(move |message: String| {
        
        String::new()
    })
}


// This can be read from a file
static INDEX_HTML: &'static [u8] = br#"
<!DOCTYPE html>
<html>
	<head>
		<meta charset="utf-8">
	</head>
	<body>
      <pre id="messages"></pre>
			<form id="form">
				<input type="text" id="msg">
				<input type="submit" value="Send">
			</form>
      <script>
        var socket = new WebSocket("ws://" + window.location.host + "/ws");
        socket.onmessage = function (event) {
          var messages = document.getElementById("messages");
          messages.append(event.data + "\n");
        };
        var form = document.getElementById("form");
        form.addEventListener('submit', function (event) {
          event.preventDefault();
          var input = document.getElementById("msg");
          socket.send(input.value);
          input.value = "";
        });
		</script>
	</body>
</html>
    "#;

// Server web application handler
struct Server {
    out: Sender,
}

impl Handler for Server {
    //
    fn on_request(&mut self, req: &Request) -> Result<(Response)> {
        // Using multiple handlers is better (see router example)
        match req.resource() {
            // The default trait implementation
            "/ws" => Response::from_request(req),

            // Create a custom response
            "/" => Ok(Response::new(200, "OK", INDEX_HTML.to_vec())),

            _ => Ok(Response::new(404, "Not Found", b"404 - Not Found".to_vec())),
        }
    }

    // Handle messages recieved in the websocket (in this case, only on /ws)
    fn on_message(&mut self, msg: Message) -> Result<()> {
        // Broadcast to all connections
        self.out.broadcast(msg)
    }
}
