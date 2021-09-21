// Functions for Reading Minecraft Packets from a TcpStream

extern crate integer_encoding;

use std::{cmp, error, fmt, io::Write, net::TcpListener, string::FromUtf8Error, todo, u64};
use std::iter::Iterator;
use std::vec::Vec;
use std::io::{self, BufRead, Read};
// use std::net::TcpStream;
// use error::Error;
// use fmt::write;
use byteorder::ReadBytesExt;
use byteorder::WriteBytesExt;
use integer_encoding::{FixedInt, FixedIntReader, VarInt, VarIntReader, VarIntWriter};
// use io::copy;
// use io::Error as IoError;

use bytes::{Bytes, BytesMut, Buf, BufMut};
// use tokio::net::TcpListener;
// use tokio::net::TcpListener;
use tokio::io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio_util::codec::{Decoder, Encoder, Framed};

use minecraft_packet_ids::*;

mod minecraft_packet_ids;
mod minecraft_packet_errors;


fn htons(n: u16) -> u16 {
    n << 8 | (n & 0xFF00) >> 8
}

fn htonl(n: u32) -> u32 {
    n << 24 | (n & 0xFF00) << 8 | (n & 0xFF0000) >> 8 | (n & 0xFF000000) >> 24
}

#[derive(Debug)]
pub enum ServerState {
    Waiting,
    Status, 
    Login,
    Play
}

#[derive(Debug)]
pub enum ChatColor {
    Black,
    DarkBlue,
    DarkGreen,
    DarkCyan,
    DarkRed,
    Purple,
    Gold,
    Gray,
    DarkGray,
    Blue,
    BrightGreen,
    Cyan,
    Red,
    Pink,
    Yellow,
    White
}


impl fmt::Display for ChatColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChatColor::Black => {f.write_str("black")}
            ChatColor::DarkBlue => {f.write_str("dark_blue")}
            ChatColor::DarkGreen => {f.write_str("dark_green")}
            ChatColor::DarkCyan => {f.write_str("dark_cyan")}
            ChatColor::DarkRed => {f.write_str("dark_red")}
            ChatColor::Purple => {f.write_str("purple")}
            ChatColor::Gold => {f.write_str("gold")}
            ChatColor::Gray => {f.write_str("gray")}
            ChatColor::DarkGray => {f.write_str("dark_gray")}
            ChatColor::Blue => {f.write_str("blue")}
            ChatColor::BrightGreen => {f.write_str("green")}
            ChatColor::Cyan => {f.write_str("cyan")}
            ChatColor::Red => {f.write_str("red")}
            ChatColor::Pink => {f.write_str("pink")}
            ChatColor::Yellow => {f.write_str("yellow")}
            ChatColor::White => {f.write_str("white")}
        }
    }
}

pub struct ChatComponent {
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub strikethrough: bool,
    pub obfuscated: bool,
    pub color: ChatColor,
    pub message: String
}

impl fmt::Display for ChatComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{\"text\":\"{}\",\"bold\":\"{}\",\"italic\":\"{}\",\"underlined\":\"{}\",\"strikethrough\":\"{}\",\"obfuscated\":\"{}\",\"color\":\"{}\"}}",
            self.message,
            self.bold,
            self.italic,
            self.underlined,
            self.strikethrough,
            self.obfuscated,
            self.color,
        )
    }
}

pub enum Chat {
    Nil,
    Cons(ChatComponent, Box<Chat>)
}

impl Chat {
    pub fn new() -> Chat {
        Chat::Nil
    }

    pub fn len(&self) -> usize {
        match self {
            Chat::Nil => 0,
            Chat::Cons(_, next) => 1 + next.len()
        }
    }

    pub fn prepend(self, c: ChatComponent) -> Chat {
        match self {
            Chat::Nil => Chat::Cons(c, Box::new(Chat::Nil)),
            Chat::Cons(cn, n) => Chat::Cons(c, Box::new(Chat::Cons(cn, n)))
        
        }
    }
    
    pub fn append(self: Chat, c: ChatComponent) -> Chat {
        match self {
            Chat::Nil => Chat::Cons(c, Box::new(Chat::Nil)),
            Chat::Cons(curr_c, next) => Chat::Cons(curr_c, Box::new(Chat::append(*next, c))) 
        }
    }
}

impl fmt::Display for Chat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Find more translation keys here: https://github.com/SpigotMC/BungeeCord/blob/master/chat/src/main/resources/mojang-translations/en_US.properties
        write!(f, "{{\"translate\":\"")?;
        for _ in 0..self.len() {
            write!(f, "%s")?;
        }
        write!(f, "\",\"with\":")?;
        write!(f, "[")?;
        match self {
            Chat::Cons(component, next) => {
                f.write_str( component.to_string().as_ref()).and_then(|_| {add_chat_components(next, f)})
            },
            Chat::Nil => add_chat_components(self, f)
        }?;

        write!(f, "}}")
    }
}

fn add_chat_components(c: &Chat, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match c {
        Chat::Cons(component, next) => write!(f, ",{}", component.to_string()).and_then(|_| {add_chat_components(next, f)}),
        Chat::Nil => write!(f, "]")
    }
}


enum RawMinecraftPacket {
    Packet {
        id: u32,
        data: Bytes,
    },
    ClassicPing {
        protocol_version: u8, 
        hostname: String,
        port: u32,  // Should fit in a u16, but client sends a u32
    }
}

pub fn try_read_varint(src: &[u8]) -> Option<u64> {
    let mut out: u64 = 0;
    let mut parts = 0;

    let MSB: u8 = 0x80;
    let DMSB: u8 = 0x7f;
    let MAX_PARTS = 5;

    for b in src.iter() {
        if parts > MAX_PARTS {
            return None
        }

        out <<= 7;
        out |= (b & DMSB) as u64;

        parts += 1;

        if b & MSB == 0 {
            return Some(out)
        }
    }

    None
}


pub struct RawMinecraftCodec<R, W> where R:AsyncRead, W:AsyncWrite {
    reader: BufReader<R>,
    writer: BufWriter<W>,
    read_buf: BytesMut
}

// impl <R, W> Decoder for RawMinecraftCodec where R:AsyncRead, W:AsyncWrite {
//     type Item = RawMinecraftPacket;
//     type Error = io::Error;

//     fn decode(
//         &mut self,
//         src: &mut BytesMut
//     ) -> Result<Option<Self::Item>, Self::Error> {
//        if src.len() == 0 {
//            Ok(None)
//        } else {
//            let mut hbuf: [u8; 1] = [0 as u8; 1];
//            self.reader.read_exact(&mut hbuf);
           
//            if hbuf[0] == 0xFE {
//                // This is a Classic Ping

               
//            }
           
//        }
//     }
// }


pub struct MinecraftPacket {
    data: Vec<u8>
}

impl MinecraftPacket {
    pub fn send<W:Write>(writer: &mut W, pkt: MinecraftPacket) -> io::Result<()> {
        // Write length header followed by pkt data
        writer.write_varint(pkt.data.len())?;
        writer.write_all(pkt.data.as_ref())
    }

    fn new_raw_packet() -> MinecraftPacket {
        MinecraftPacket {data: Vec::new()}
    }

    fn new(ptype: ClientboundPacket) -> io::Result<MinecraftPacket>{ // Change to support all types of packets
        let mut pkt = MinecraftPacket::new_raw_packet();

        match ptype {
            ClientboundPacket::LoginPacket(l) => pkt.data.write_varint(l as u16)?,
            ClientboundPacket::StatusPacket(s) => pkt.data.write_varint(s as u16)?,
        };

        Ok(pkt)
    }

    pub fn new_disconnect_packet(message: &Chat) -> io::Result<MinecraftPacket>{
        MinecraftPacket::new(LoginPacketResponse::Disconnect)?.append_string(
            message.to_string().as_ref()
        )
    }

    // pub fn new_pong_packet(nonce: i64) {
    //     MinecraftPacket::new(StatusPacketResponse::Pong{payload: nonce})
    // }


    fn append_string(mut self, string: &str) -> io::Result<MinecraftPacket> {
        let string_bytes = string.as_bytes();
        self.data.write_varint(string_bytes.len())?;
        self.data.extend_from_slice(string_bytes);

        Ok(self)
    }
}

pub struct MinecraftPacketWriter<T> where T:Write {
    stream: T,
}

impl<T> MinecraftPacketWriter<T> where T:Write {
    pub fn new(mut stream: T) -> MinecraftPacketWriter<T> {
        MinecraftPacketWriter {stream}
    }

    fn send_packet(&mut self, packet_id: u8, data: &[u8]) -> io::Result<()> {
        let packet_len = 1 + data.len();
        self.stream.write_varint(packet_len)?;
        self.stream.write_u8(packet_id & 0x7f)?;
        self.stream.write_all(data)
    }

    fn send_str(&mut self, data: &str) -> io::Result<usize> {
        let str_bytes = data.as_bytes();
        let data_len = str_bytes.len();
        let size_header_size = self.stream.write_varint(data_len)?;
        self.stream.write_all(str_bytes)?;
        
        Ok(size_header_size + data_len)
    }

    pub fn send_disconnect(&mut self, message: &Chat) {
        let msg_json: String = message.to_string();
        self.send_packet(LoginPacketResponse::Disconnect as u8, msg_json.as_bytes());
    }
}


pub struct MinecraftPacketReader<R:Read> {
    reader: io::BufReader<R>,
    state: ServerState,
}

impl<R> VarIntReader for MinecraftPacketReader<R> where R:Read {
    fn read_varint<VI: VarInt>(&mut self) -> io::Result<VI> {
        match self {
            MinecraftPacketReader { reader, .. } => reader.read_varint(),
        }
    }
}

impl<R> Iterator for MinecraftPacketReader<R> where R:Read {
    type Item = ServerboundPacket;
    fn next(&mut self) -> Option<ServerboundPacket> {
        let mut hbuf: [u8; 1] = [0 as u8; 1];
        self.reader.read_exact(&mut hbuf);
    
        if hbuf[0] == 0xFE {
            // This is a Classic Ping
            self.next_classic_mc_packet()
        }
        else {
            // Remove remainder of length VarInt (it is unused anyway)
            if hbuf[0] & 0x80 != 0 {
                let _packet_len = self.reader.read_varint::<u32>();
            }

            self.next_mc_packet()
        }
    }
 
}

impl<R> MinecraftPacketReader<R> where R:Read {
    pub fn new(stream: R) -> io::Result<Self> {
        let reader = io::BufReader::new(stream);
        Ok(Self {reader: reader, state: ServerState::Waiting})
    }

    pub fn set_state(&mut self, state: ServerState) {
        self.state = state;
    }


    fn read_uchar(&mut self) -> io::Result<u8> {
        self.reader.read_u8()
    }

    fn read_ushort(&mut self) -> io::Result<u16> {
        self.reader.read_fixedint()
    }

    fn read_uint(&mut self) -> io::Result<u32> {
        self.reader.read_fixedint()
    }

    fn read_long(&mut self) -> io::Result<i64> {
        self.reader.read_fixedint()
    }

    fn read_utf_string(&mut self, max_bytes: usize) -> Result<String, minecraft_packet_errors::MinecraftStringError>{
        // let mut str_bytes: Vec<u8> = self.reader.by_ref().bytes().take(6).map(|c| c.unwrap()).collect::<Vec<u8>>();
        let str_len_bytes = cmp::min(max_bytes, self.read_varint::<usize>()?);
        let str_bytes: Result<Vec<u8>, _> = self.reader.by_ref().bytes().take(str_len_bytes).collect();
        Ok(String::from_utf8(str_bytes?)?) // Change error return type so this can't panic

    }

    fn read_utf16_string(&mut self, len_bytes: usize) -> Result<String, minecraft_packet_errors::MinecraftStringError>{
        let str_bytes: Result<Vec<u8>, _> = self.reader.by_ref().bytes().take(len_bytes).collect();
        let str_hwords = str_bytes?.chunks_exact(2).into_iter().map(|hw| u16::from_ne_bytes([hw[1], hw[0]])).collect::<Vec<u16>>();
        Ok(String::from_utf16(&str_hwords)?)
    //     // let mut str_bytes: Vec<u8> = self.reader.by_ref().bytes().take(6).map(|c| c.unwrap()).collect::<Vec<u8>>();
    //     let str_len_bytes = cmp::min(max_bytes, self.read_varint::<usize>()?);
    //     let str_bytes: Result<Vec<u8>, _> = self.reader.by_ref().bytes().take(str_len_bytes).collect();
    //     let str_hwords = str_bytes?.chunks_exact(2).into_iter().map(|hw| u16::from_ne_bytes([hw[0], hw[1]])).collect::<Vec<u16>>();
    //     Ok(String::from_utf16(&str_hwords)?) // Change error return type so this can't panic

    }




    pub fn read_handshake_packet(&mut self) -> io::Result<ServerboundPacket> {
        let protocol_version = self.read_varint()?;
        println!("proto_version: {}", protocol_version);
        let server_address: String = self.read_utf_string(255 * 4).unwrap(); // 4 bytes in max UTF-8 char
        println!("server_address: {}", server_address);
        let server_port = self.read_ushort()?;
        println!("server_port: {}", server_port);
        let next_state_num = self.reader.read_varint::<u8>().unwrap();
        println!("next_state_num: {}", next_state_num);
        let next_state = match next_state_num {
            1 => ServerState::Status,
            2 => ServerState::Login,
            _ => panic!("Client asked server to switch into an invalid state."),  // Make this its own type
        };
        Ok(ServerboundPacket::SetState {protocol_version, server_address, server_port, next_state})
    }

    pub fn read_login_start_packet(&mut self) -> io::Result<ServerboundPacket> {
        let username = self.read_utf_string(16 * 4).unwrap();  // TODO: proper error handling
        Ok(ServerboundPacket::StartLogin {username})
    }

    pub fn read_encryption_response_packet(&mut self) -> io::Result<ServerboundPacket> {
        let shared_secret_length = self.reader.read_varint::<usize>()?;
        let shared_secret: Vec<u8> = self.reader.by_ref().bytes().take(shared_secret_length).collect::<io::Result<Vec<u8>>>()?;

        let verify_token_length = self.reader.read_varint::<usize>()?;
        let verify_token: Vec<u8> = self.reader.by_ref().bytes().take(verify_token_length).collect::<io::Result<Vec<u8>>>()?;

        Ok(ServerboundPacket::EncryptionResponse {shared_secret, verify_token})
    }

    pub fn read_plugin_response_packet(&mut self) -> io::Result<ServerboundPacket> {
        todo!()
    }

    pub fn read_ping_packet(&mut self) -> io::Result<ServerboundPacket> {
        let nonce: i64 = self.read_long()?;

        Ok(ServerboundPacket::Ping {payload: nonce})
    }

    pub fn read_classic_ping_packet(&mut self) -> io::Result<ServerboundPacket> {
        println!("Classic Ping");
        let payload = self.read_uchar()?;
        let plugin_msg_id = self.read_uchar()?;

        if payload != 1 || plugin_msg_id != 0xFA {
            println!("Classic Ping's payload was {} instead of 1 or plugin_msg_id was {} instead of 0xFA", payload, plugin_msg_id);
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Classic Packet bad payload or plugin+msg+id"));
        }


        let ping_host_len = htons(self.read_ushort()?);

        if ping_host_len != 11 {
            println!("Classic Ping's ping_host_len was {} instead of 11", ping_host_len);
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Classic Ping's ping_host_len was {} instead of 11"));
        }

        // TODO: Handle error
        let ping_host_str: String = self.read_utf16_string(2 * 11).unwrap();

        if ping_host_str != "MC|PingHost" {
            println!("Classic Ping's ping_host_str was {:?} instead of 'MC|PingHost'", ping_host_str);
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Classic Ping's ping-host_str wasn't 'MC|PingHost'"));
        }

        let _data_len = htons(self.read_ushort()?);
        let protocol_version = self.read_uchar()?;
        let hostname_len = htons(self.read_ushort()?);
        // TODO: Be careful here reading n bytes set by client.  Will probably just timeout if client sends large number?
        // TODO: Handle error
        let hostname: String = self.read_utf16_string(2 * hostname_len as usize).unwrap();  
        let server_port: u32 = htonl(self.read_uint()?);

        Ok(ServerboundPacket::ClassicPing{protocol_version, hostname, server_port})
    }

    fn next_classic_mc_packet(&mut self) -> Option<ServerboundPacket> {
        self.read_classic_ping_packet().ok()
    }
    
    fn next_mc_packet(&mut self) -> Option<ServerboundPacket>{
        // let _packet_len = self.read_varint::<u32>();
        let packet_id = self.read_varint::<u32>();
        
        match self.state {
            ServerState::Waiting => {
                match packet_id {
                    Ok(0u32) => self.read_handshake_packet().ok(),
                    Ok(id) => {
                        let mut buf: [u8; 10] = [0;10];
                        println!("Unexpected Packet ID: {:X}", id);
                        self.reader.read(&mut buf).unwrap();
                        println!("Body: '{:?}'", buf);
                        None
                    }
                    Err(_) => {
                        println!("Malformed Packet ID!");
                        None
                    }
                }
            },
            ServerState::Login => {
                match packet_id {
                    Ok(0u32) => self.read_login_start_packet().ok(),
                    Ok(1u32) => self.read_encryption_response_packet().ok(),
                    Ok(2u32) => self.read_plugin_response_packet().ok(),
                    Ok(id) => {
                        println!("Unexpected Packet ID: {}", id);
                        None
                    },
                    Err(_) => {
                        println!("Malformed Packet ID!");
                        None
                    }
                }
            }, 
            ServerState::Status => {
                match packet_id {
                    Ok(0u32) => panic!("Status request not supported yet."),
                    Ok(1u32) => self.read_ping_packet().ok(),
                    Ok(id) => {
                        println!("Unexpected Packet ID: {}", id);
                        None
                    }
                    Err(_) => {
                        println!("Malformed Packet ID");
                        None
                    }

                }
            },
            ServerState::Play => {
                panic!("Play state not supported.")
            }
        }
    }
}


// Packet IDs valid without a context.
// Serverbound
#[derive(Debug)]
pub enum ServerboundPacket {
    // AKA Handshake.  Switches server to desired state
    SetState {  
        protocol_version: u16,  // Game Version used to connect.  https://wiki.vg/Protocol_version_numbers
        server_address: String,  // Address of server client is connecting to
        server_port: u16,
        next_state: ServerState,
    },

    // Packets valid in the Login state
    StartLogin {
        username: String
    },
    EncryptionResponse {
        shared_secret: Vec<u8>,
        verify_token: Vec<u8>
    },
    LoginPluginResponse {
        message_id: u64,
        successful: bool,
        data: Option<Vec<u8>>
    },

    // Packets Valid in the Status state
    // Client wants a JSON summary of server version, current players, max players, description, ico
    StatusRequest {
        // No fields
    },

    // Client wants a pong
    Ping {
        payload: i64  // Number to be echoed to the client
    },

    ClassicPing {
        protocol_version: u8,  // Last version was 0x4A (not a short like newer SetState packet)
        hostname: String,
        server_port: u32,  // Sent by client as int.  Should be able to fit in u16
    }

}

// Clientbound
enum HandshakePacketResponse {
    // There are none.  Handshake always transitions to another state.
}

// Clientbound
enum StatusPacketResponse {
    // JSON summary of server version, current players, max players, description, icon 
    StatusResponse,  
    
    // Response to a client ping
    Pong {
        payload: i64 // Number from the client Ping
    }, 
}


// Clientbound
enum LoginPacketResponse {
    Disconnect, // Disconnects client with reason
    EncryptionRequest, // Asks client to setup encrypted session with PublicKey and random VerifyToken
    LoginSuccess, // Acknowledge login success & switch to PLAY state by sending player username and UUID
}

enum ClientboundPacket {
    // Status Responses
    // JSON summary of server version, current players, max players, description, icon 
    StatusResponse,  
    
    // Response to a client ping
    Pong {
        payload: i64 // Number from the client Ping
    }, 

    // Login Responses
    Disconnect, // Disconnects client with reason
    EncryptionRequest, // Asks client to setup encrypted session with PublicKey and random VerifyToken
    LoginSuccess, // Acknowledge login success & switch to PLAY state by sending player username and UUID
}

impl ClientboundPacket {
    fn packet_id(&mut self) -> u16 {
        self as u16
    }
}

impl From<ClientboundPacket> for u16 {
    fn from(item: ClientboundPacket) -> Self {
        match item {
            ClientboundPacket::StatusResponse{..} => 0,
            ClientboundPacket::Pong{..} => 1,

            ClientboundPacket::Disconnect{..} => 0,
            ClientboundPacket::EncryptionRequest{..} => 1,
            ClientboundPacket::LoginSuccess{..} => 2,

        }
    }
}

