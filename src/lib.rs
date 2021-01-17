#[macro_use]
extern crate vst;
extern crate time;
extern crate rand;
extern crate rand_distr;
extern crate ws;
extern crate webbrowser;

use vst::buffer::AudioBuffer;
use vst::plugin::{Category, Info, Plugin, PluginParameters};
use vst::util::AtomicFloat;
use vst::api;
use vst::buffer::{ SendEventBuffer};
use vst::editor::Editor;
use vst::event::{Event, MidiEvent};
use vst::plugin::{CanDo, HostCallback,};
use std::sync::Arc;
use rand_distr::{Normal, Distribution};
use std::thread;
use std::time::Duration;
use std::os::raw::c_void;

/// An example of a chat web application server
use ws::{listen, Handler, Message, Request, Response, Result, Sender};

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


/**
 * Parameters
 */ 
struct UclidParameters {
    variance: AtomicFloat,
}


impl Default for UclidParameters {
    fn default() -> UclidParameters {
        UclidParameters {
            variance: AtomicFloat::new(0.0),
        }
    }
}


static MAX_VARIANCE: f32 = 25.;


/**
 * Plugin
 */ 

#[derive(Default)]
struct Uclid {
    host: HostCallback,
    sample_rate: f32,
    immediate_events: Vec<MidiEvent>,
    send_buffer: SendEventBuffer,
    params: Arc<UclidParameters>,
}


impl Uclid {
    fn add_event(&mut self, e: MidiEvent) {
        let velocity = e.data[2];
        let variance = self.params.variance.get() * MAX_VARIANCE;

        let normal = Normal::new(velocity as f32, variance).unwrap();
        let v = normal.sample(&mut rand::thread_rng()).max(0.).min(127.) as f32;

        self.immediate_events.push(MidiEvent {
            data: [e.data[0], e.data[1], v as u8],
            ..e
        });
    }
    
    fn send_midi(&mut self) {
        // Immediate
        self.send_buffer.send_events(&self.immediate_events, &mut self.host);
        self.immediate_events.clear();
    }
}


struct BrowserTabEditor {

}

impl BrowserTabEditor {

}

impl Editor for BrowserTabEditor {
    fn size(&self) -> (i32, i32) {
        (500,500)
    }

    fn position(&self) -> (i32, i32) {
        (500,500)
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

impl Plugin for Uclid {
    fn new(host: HostCallback) -> Self {
        let mut p = Uclid::default();
        p.host = host;
        p.params = Arc::new(UclidParameters::default());
        
        

        p
    }

    fn get_info(&self) -> Info {
        
        Info {
            name: "Uclid".to_string(),
            vendor: "Rein van der Woerd".to_string(),
            unique_id: 298467,
            version: 1,
            inputs: 2,
            outputs: 2,
            // This `parameters` bit is important; without it, none of our
            // parameters will be shown!
            parameters: 1,
            category: Category::Effect,
            ..Default::default()
        }
    }

    fn set_sample_rate(&mut self, rate: f32) {
        self.sample_rate = rate;
    }

    fn process_events(&mut self, events: &api::Events) {
        for e in events.events() {
            #[allow(clippy::single_match)]
            match e {
                Event::Midi(e) => self.add_event(e),
                _ => (),
            }
        }
    }


    fn process(&mut self, buffer: &mut AudioBuffer<f32>) {
        for (input, output) in buffer.zip() {
            for (in_sample, out_sample) in input.iter().zip(output) {
                *out_sample = *in_sample;
            }
        }
        self.send_midi();
    }

    fn can_do(&self, can_do: CanDo) -> vst::api::Supported {
        use vst::api::Supported::*;
        use vst::plugin::CanDo::*;

        match can_do {
            SendEvents | SendMidiEvent | ReceiveEvents | ReceiveMidiEvent => Yes,
            _ => No,
        }
    }


    // Return the parameter object. This method can be omitted if the
    // plugin has no parameters.
    fn get_parameter_object(&mut self) -> Arc<dyn PluginParameters> {
        Arc::clone(&self.params) as Arc<dyn PluginParameters>
    }
    
    fn get_editor(&mut self) -> Option<Box<dyn Editor>> {
        Some(Box::new(BrowserTabEditor {}))
    }
}

impl PluginParameters for UclidParameters {
    // the `get_parameter` function reads the value of a parameter.
    fn get_parameter(&self, index: i32) -> f32 {
        match index {
            0 => self.variance.get(),
            _ => 0.0,
        }
    }

    // the `set_parameter` function sets the value of a parameter.
    fn set_parameter(&self, index: i32, val: f32) {
        #[allow(clippy::single_match)]
        match index {
            0 => self.variance.set(val.max(0.0000000001)),
            _ => (),
        }
    }

    // This is what will display underneath our control.  We can
    // format it into a string that makes the most since.
    fn get_parameter_text(&self, index: i32) -> String {
        match index {
            0 =>  format!("{:.1}", self.variance.get() * MAX_VARIANCE),
            _ => "".to_string(),
        }
    }

    // This shows the control's name.
    fn get_parameter_name(&self, index: i32) -> String {
        match index {
            0 => "Velocity variance",
            _ => "",
        }
        .to_string()
    }
}

// This part is important!  Without it, our plugin won't work.
plugin_main!(Uclid);