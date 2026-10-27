//! Native Sciter fixture. Uses real panel markup/CSS/TIS with an inert session handler.
use std::path::Path;
use sciter::dom::event::BEHAVIOR_EVENTS;

fn eval(window: &sciter::Window, script: &str) -> bool {
    window.get_host().eval_script(script).unwrap_or_else(|e| panic!("{script}: {e:?}")).to_bool().unwrap_or(false)
}
fn click(window: &sciter::Window, id: &str) {
    window.get_host().get_root().unwrap().find_first(&format!("#{id}"))
        .unwrap().unwrap().send_event(BEHAVIOR_EVENTS::BUTTON_CLICK, None, None).unwrap();
}
fn measure_width(window: &sciter::Window, id: &str) -> i32 {
    let root=window.get_host().get_root().unwrap(); root.update(true).unwrap();
    let el=root.find_first(&format!("#{id}")).unwrap().unwrap();
    let rect=el.get_location(sciter::dom::ELEMENT_AREAS::BORDER_BOX as u32).unwrap();
    rect.right-rect.left
}
fn main() {
    let dll=std::env::args().nth(1).expect("sciter.dll");
    sciter::set_library(Path::new(&dll).to_str().unwrap()).unwrap();
    let html=std::fs::read_to_string("src/ui/remote.html").unwrap();
    let panel=html.split("<section #agent-panel>").nth(1).unwrap().split("</section>").next().unwrap();
    let css=std::fs::read_to_string("src/ui/remote.css").unwrap();
    let common=std::fs::read_to_string("src/ui/common.css").unwrap();
    let script=std::fs::read_to_string("src/ui/agent.tis").unwrap();
    let stub=r#"
        var state={mode:"Manual",connected:true,mcpReady:true,manualPasteAllowed:true,peer:"fixture",status:"Вы управляете"};
        var commands=[]; var pasted=0; var adapted=0; var denyStart=false;
        var keyboard_enabled=true; var is_file_transfer=false; var is_port_forward=false; var entered=false;
        function scaleIt(n) {return n;} function adaptDisplay(){adapted++;}
        var handler={agent_status:function(){return JSON.stringify(state);},agent_fixture:function(){return false;},
            leave:function(m){}, get_keyboard_mode:function(){return "legacy";},
            agent_control:function(c,t){if(c=="start" && denyStart)return "Нет актуального изображения";
                commands.push(c);state.mode=c=="start"?"Running":"Manual";
                state.status=c=="start"?"Управляет Codex":"Вы управляете";return "";},
            agent_paste:function(){pasted++;return "";}};
    "#;
    for (width,height) in [(1280,800),(900,600),(850,500)] {
        for theme in ["", "darktheme"] {
            let document=format!(r#"<html class="{theme}"><head><meta charset="utf-8"><style>{common}{css}</style>
                <script type="text/tiscript">{stub}{script}</script></head>
                <body><div id="video-wrapper"></div><section id="agent-panel">{panel}</section></body></html>"#);
            let mut window=sciter::WindowBuilder::popup().with_size((width,height)).create();
            assert!(window.load_html(document.as_bytes(),None));
            assert!(eval(&window,"agentElement('agent-start').text == 'Передать Codex'"));
            assert!(eval(&window,"!agentElement('agent-paste').state.disabled"));
            let expanded=measure_width(&window,"agent-panel");
            let video_expanded=measure_width(&window,"video-wrapper");
            click(&window,"agent-start");
            assert!(eval(&window,"commands.length==1 && commands[0]=='start' && agentElement('agent-start').text=='Вернуть управление'"));
            assert!(eval(&window,"agentElement('agent-paste').state.disabled"));
            click(&window,"agent-collapse");
            assert!(eval(&window,"agentCollapsed && agentPanel.attributes['collapsed']=='true' && agentElement('agent-reclaim').style['display']=='block' && commands.length==1"));
            let collapsed=measure_width(&window,"agent-panel");
            assert!(collapsed <= 40 && expanded-collapsed >= 190, "panel: {expanded} -> {collapsed}");
            assert!(measure_width(&window,"video-wrapper")-video_expanded >= 190);
            click(&window,"agent-reclaim");
            assert!(eval(&window,"state.mode=='Manual' && commands[1]=='manual' && agentElement('agent-reclaim').style['display']=='none'"));
            click(&window,"agent-collapse");
            assert!(eval(&window,"!agentCollapsed && agentPanel.attributes['collapsed']=='false'"));
            assert_eq!(measure_width(&window,"agent-panel"),expanded);
            click(&window,"agent-paste");
            assert!(eval(&window,"pasted==1"));
            assert!(eval(&window,"state.connected=false;updateAgent();agentElement('agent-start').state.disabled && agentElement('agent-paste').state.disabled"));
            assert!(eval(&window,"state.mode='Paused';updateAgent();!agentElement('agent-start').state.disabled && agentElement('agent-start').text=='Вернуть управление'"));
            click(&window,"agent-start");
            assert!(eval(&window,"state.mode=='Manual' && commands[2]=='manual'"));
            assert!(eval(&window,"state.connected=true;denyStart=true;updateAgent();true"));
            click(&window,"agent-start");
            assert!(eval(&window,"state.mode=='Manual' && agentElement('agent-start').text=='Передать Codex' && agentElement('agent-feedback').text=='Нет актуального изображения'"));
            click(&window,"agent-viewer");assert!(eval(&window,"agentElement('video-wrapper').style['visibility']=='hidden'"));
            click(&window,"agent-viewer");assert!(eval(&window,"agentElement('video-wrapper').style['visibility']=='visible'"));
            assert!(eval(&window,"is_file_transfer=true;updateAgent();agentPanel.style['display']=='none'"));
            println!("native controls {width}x{height} theme={theme:?}: PASS panel={expanded}->{collapsed}");
        }
    }
}
