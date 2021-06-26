use std::collections::VecDeque;
extern crate byteorder;
extern crate packet;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use std::net::IpAddr;

fn main() {
    let udp_packet = UDPHeader {
        source_port: 0,
        destination_port: 0,
        length: 0,
        checksum: 0,
    }
    .make_udp_packet(vec![1, 2]);
    println!("{}", udp_packet.len());
    let mut result = Vec::new();
    for i in (0..udp_packet.len()).step_by(2) {
        result.push(BigEndian::read_u16(&udp_packet[i..])); // <-- this hear converts everything
                                                            // back to u16, but the data section is already u8 to start with so probably should only
                                                            // go the length of the udp header.
    }
    println!("{:?}", result);
}

pub trait Protocol {
    type Address;
    type Connection; // <--- In layman terms what would a connection be.
    type Data; // Data here will be a packet, so the send function will probably make
               // a packet and then send it on to the lower levels.
    fn connect(address: Self::Address) -> Self::Connection;
    fn receive(connection: &Self::Connection) -> Self::Data;
    fn send(connection: &Self::Connection, data: &Self::Data);
}

pub trait Transport: Protocol {
    type Port;
    type Checksum;
    fn error_check(checksum: &Self::Checksum) -> bool;
}

impl Protocol for UDP {
    type Address = u16;
    type Connection = UDPConnection;
    type Data = Vec<u8>;

    fn connect(address: Self::Address) -> Self::Connection {
        todo!()
    }

    fn receive(connection: &Self::Connection) -> Self::Data {
        todo!()
    }

    fn send(connection: &Self::Connection, data: &Self::Data) {
        todo!()
    }
}

pub struct UDPConnection {
    connection: IPConnection,
    source_port: u16,
    destination_port: u16,
}
pub struct IPAddress {
    octect1: u8,
    octect2: u8,
    octect3: u8,
    octect4: u8,
}
pub struct IPConnection {
    connection: EtherConnection,
    source_IP: IPAddress,
    destination_IP: IPAddress,
}

// so we first establish a connection with the other side, and that is done with these structs.
// Then this is passed into the send and recieve and i assume it helps inform each level what
// addresses it needs?

pub struct EtherConnection {}

pub trait Network: Protocol {}

pub trait Link: Protocol {}

pub struct UDP {}

// Should the size limit of UDP be set into the protocol?
pub struct UDPHeader {
    source_port: u16,
    destination_port: u16,
    length: u16,
    checksum: u16,
}

impl UDPHeader {
    // Alright, so I am assuming the byte type in rust is u8? Or should it be i8?
    pub fn make_udp_packet(&mut self, data: Vec<u8>) -> Vec<u8> {
        //let mut udp_packet = VecDeque::new();
        let mut udp_packet = Vec::new();
        self.length = (8 + data.len()) as u16;

        // let source_port_bytes = source_port.to_be_bytes();
        // let destination_port_bytes = destination_port.to_be_bytes()[0];

        // WHAT IS THE MEANING OF GLOBAL?

        // udp.write_u16::<BigEndian>(source_port).unwrap();
        // println!("{:?}", udp);

        // THE DATA READ IN IS NOT REPRESENTED AS BYTES?
        udp_packet.write_u16::<BigEndian>(self.source_port).unwrap();
        udp_packet
            .write_u16::<BigEndian>(self.destination_port)
            .unwrap();
        udp_packet.write_u16::<BigEndian>(self.length).unwrap();
        for i in data {
            udp_packet.write_u8(i).unwrap(); // <-- Extend vs push, why does one work and
                                             // another
                                             // not?
        }
        udp_packet
    }
}

// Some things here are not exact lengths, so I gave it the smallest largest size possible, so
// that we can fit the data in at least, i.e. flags needs 3 bytes, given 8. Issue with this is
// enforcement, things that can be a maximum of 1 bit are not 8 bits etc
pub struct IPHeader {
    version: u8,
    IHL: u8,
    DSCP: u8,
    ECN: u8,
    total_length: u16,
    id: u16,
    flags: u8,
    fragment_offset: u16,
    ttl: u8,
    protocol: u8,
    header_checksum: u16,
    source_ip_address: u32,
    destination_ip_address: u32,
}
impl IPHeader {
    pub fn make_ip_packet(&mut self, data: Vec<u8>) -> Vec<u8> {
        unimplemented!()
    }
}
pub struct EtherHeader {
    preamble: u64,
    sfd: u8,
    destination_address: u64,
    source_address: u64,
    length: u16,
    crc: u32,
}

impl EtherHeader {
    pub fn make_ether_packet() -> Vec<u8> {
        unimplemented!()
    }
}
