// Minecraft Packet IDs


// Packet IDs valid without a context.
// Serverbound
pub enum HandshakePacketIds {
    SetState,  // AKA Handshake.  Transitions server to desired state

}

// Clientbound
pub enum HandshakePacketResponseIds {
    // There are none.  Handshake always transitions to another state.
}


// Packet IDs valid in the GetServerStatus context
// Serverbound
pub enum StatusPacketIds {
    StatusRequest,  // Client wants a JSON summary of server version, current players, max players, description, icon
    Ping, // Client wants a pong
}

// Clientbound
pub enum StatusPacketResponseIds {
    StatusResponse,  // JSON summary of server version, current players, max players, description, icon 
    Pong, // Response to a client ping
}


// Packet IDs valid in the Login context
// Serverbound
pub enum LoginPacketIds {
    StartLogin,
    EncryptionResponse,
    LoginPluginResponse,
}

// Clientbound
pub enum LoginPacketResponseIds {
    Disconnect, // Disconnects client with reason
    EncryptionRequest, // Asks client to setup encrypted session with PublicKey and random VerifyToken
    LoginSuccess, // Acknowledge login success & switch to PLAY state by sending player username and UUID
}