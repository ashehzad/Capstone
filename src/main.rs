use std::collections::VecDeque;
extern crate byteorder;
extern crate packet;
extern crate pnet;
extern crate tokio;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use pnet::util::MacAddr;
use std::marker::PhantomData;
use std::net::{IpAddr, Ipv4Addr};

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
    let udp = packet::udp::Builder::default().destination(16);
    let udpp = pnet::packet::udp::Udp {
        source: 1345,
        destination: 0,
        length: 0,
        checksum: 0,
        payload: vec![],
    };
    println!("{:?}", result);
}

pub trait Protocol {
    type Address;
    type Connection;
    type Data;
    fn connect(
        &self,
        source_address: Self::Address,
        destination_address: Self::Address,
    ) -> Self::Connection;

    fn receive(connection: &Self::Connection) -> Self::Data;
    fn send(connection: &Self::Connection, data: &Self::Data);
}
pub trait Transport: Protocol {
    fn listen(&self, address: Self::Address) -> Box<dyn Iterator<Item = Self::Connection>>;
}

impl<P: Protocol> Protocol for UDP<P> {
    type Address = (P::Address, u16);
    type Connection = UDPConnection<P>;
    type Data = Vec<u8>;

    fn connect(
        &self,
        (source_address, source_port): Self::Address,
        (destination_address, destination_port): Self::Address,
    ) -> Self::Connection {
        UDPConnection {
            connection: self
                .inner_protocol
                .connect(source_address, destination_address),
            source_port,
            destination_port,
        }
    }

    fn receive(connection: &Self::Connection) -> Self::Data {
        todo!()
    }

    fn send(connection: &Self::Connection, data: &Self::Data) {
        todo!()
    }
}

impl<P: Protocol> Transport for UDP<P> {
    fn listen(&self, address: Self::Address) -> Box<dyn Iterator<Item = Self::Connection>> {
        todo!()
    }
}

pub struct UDPConnection<P: Protocol> {
    connection: P::Connection,
    source_port: u16,
    destination_port: u16,
}

pub struct IPConnection {
    connection: EtherConnection,
    source_ip: Ipv4Addr,
    destination_ip: Ipv4Addr,
}

pub struct EtherConnection {
    source_mac: MacAddr,
    destination_mac: MacAddr,
    sender: tokio::sync::broadcast::Sender<Vec<u8>>,
    receiver: tokio::sync::broadcast::Receiver<Vec<u8>>,
}

pub trait Network: Protocol {}

pub trait Link: Protocol {}

pub struct Ether {
    sender: tokio::sync::broadcast::Sender<Vec<u8>>,
    receiver: tokio::sync::broadcast::Receiver<Vec<u8>>,
}

pub struct UDP<P> {
    inner_protocol: P,
}

// Should the size limit of UDP be set into the protocol?
pub struct UDPHeader {
    source_port: u16,
    destination_port: u16,
    length: u16,
    checksum: u16,
}

impl UDPHeader {
    pub fn make_udp_packet(&mut self, data: Vec<u8>) -> Vec<u8> {
        //let mut udp_packet = VecDeque::new();
        let mut udp_packet = Vec::new();
        self.length = (8 + data.len()) as u16;

        //let destination_port_bytes = udp_packet.extend(self.destination_port.to_be_bytes());

        // WHAT IS THE MEANING OF GLOBAL?

        // udp.write_u16::<BigEndian>(source_port).unwrap();
        // println!("{:?}", udp);

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
