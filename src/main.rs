use std::thread;
use std::net::{TcpListener, TcpStream, Shutdown};
use std::io::{Read, Write};
use std::thread::sleep_ms;

use minecraft_packet::{MinecraftPacket, MinecraftPacketReader, MinecraftPacketWriter, Chat, ChatComponent, ChatColor};
// use std::result::Result;

mod minecraft_packet;



fn handle_client(mut stream: TcpStream) {
    println!("Client connected!");
    // let mut writer = MinecraftPacketWriter::new(stream);

    let disconnect_msg = Chat::new()
    .append(
        ChatComponent {
            bold: false,
            italic: false,
            underlined: false,
            strikethrough: false,
            obfuscated: true,
            color: ChatColor::BrightGreen,
            message: "ooo ".to_owned()
        }
    ).append(
        ChatComponent {
            bold: false,
            italic: true,
            underlined: false,
            strikethrough: false,
            obfuscated: false,
            color: ChatColor::DarkRed,
            message: "Thanks ".to_owned()
        }
    ).append(
        ChatComponent {
            bold: false,
            italic: false,
            underlined: false,
            strikethrough: false,
            obfuscated: false,
            color: ChatColor::Pink,
            message: "for ".to_owned()
        }
    ).append(
        ChatComponent {
            bold: false,
            italic: false,
            underlined: false,
            strikethrough: false,
            obfuscated: false,
            color: ChatColor::Gold,
            message: "the ".to_owned()
        }
    ).append(
        ChatComponent {
            bold: true,
            italic: true,
            underlined: true,
            strikethrough: false,
            obfuscated: false,
            color: ChatColor::DarkRed,
            message: "Rust".to_owned()
        }
    ).append(
        ChatComponent {
            bold: true,
            italic: false,
            underlined: false,
            strikethrough: false,
            obfuscated: false,
            color: ChatColor::Blue,
            message: " reading!".to_owned()
        }
    ).append(
        ChatComponent {
            bold: false,
            italic: false,
            underlined: false,
            strikethrough: false,
            obfuscated: true,
            color: ChatColor::BrightGreen,
            message: " ooo".to_owned()
        }
    );

    let mut disconnect_pkt = MinecraftPacket::new_disconnect_packet(&disconnect_msg).unwrap();
    let mut reader = minecraft_packet::MinecraftPacketReader::new(stream.try_clone().unwrap()).unwrap();
    let handshake = reader.next();
    println!("Received Handshake Packet: {:?}", handshake);
    if let minecraft_packet::ServerboundPacket::SetState{next_state, ..} = handshake.unwrap() {
        match next_state {
            minecraft_packet::ServerState::Login => {
                reader.set_state(minecraft_packet::ServerState::Login);
                let login_start = reader.next();
                println!("Received Login Start Packet: {:?}", login_start);
            }
            minecraft_packet::ServerState::Status => {
                reader.set_state(minecraft_packet::ServerState::Status);
                let status_pkt = reader.next();
                println!("Received Status Packet: {:?}", status_pkt);
            }

            minecraft_packet::ServerState::Play => {
                panic!("Play state is unsupported!");
            }

            minecraft_packet::ServerState::Waiting => {
                panic!("Client should never request a transition INTO the WAITING state!")
            }
        }
    }
    else {
        panic!("Unsupported packet!");
    }


    println!("Sending Disconnect");
    MinecraftPacket::send(&mut stream, disconnect_pkt);

    // MinecraftPacket::send(disconnect_pkt, stream);
    // writer.send_disconnect(&disconnect_msg);


    // let mut data = [0 as u8; 50]; // using 50 byte buffer
    // while match stream.read(&mut data) {
    //     Ok(size) => {
    //         // echo everything!
    //         stream.write(&data[0..size]).unwrap();
    //         true
    //     },
    //     Err(_) => {
    //         println!("An error occurred, terminating connection with {}", stream.peer_addr().unwrap());
    //         stream.shutdown(Shutdown::Both).unwrap();
    //         false
    //     }
    // } {}
}

fn main() {

    let listener = TcpListener::bind("0.0.0.0:25565").unwrap();
    // accept connections and process them, spawning a new thread for each one
    println!("Server listening on port 25565");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("New connection: {}", stream.peer_addr().unwrap());
                thread::spawn(move|| {
                    // connection succeeded
                    handle_client(stream)
                });
            }
            Err(e) => {
                println!("Error: {}", e);
                /* connection failed */
            }
        }
    }
    // close the socket server
    drop(listener);
}