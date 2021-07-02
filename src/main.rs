use async_trait::async_trait;
use pnet_base::MacAddr;
use pnet_packet::Packet;
use std::net::Ipv4Addr;
use tokio::sync::broadcast;

#[tokio::main]
async fn main() {
    // let udp = packet::udp::Builder::default().destination(16);
    // let udpp = pnet::packet::udp::Udp {
    //     source: 1345,
    //     destination: 0,
    //     length: 0,
    //     checksum: 0,
    //     payload: vec![],
    // };

    // let (tx, mut rx1) = broadcast::channel(16);

    // let computer_1 = Ether {
    //     sender: (),
    //     receiver: (),
    // };
}
#[async_trait]
pub trait Protocol {
    type Address;
    type Connection: Send + Sync;
    type Data;
    fn connect(
        &self,
        source_address: Self::Address,
        destination_address: Self::Address,
    ) -> Self::Connection;
    async fn receive(connection: &mut Self::Connection) -> Self::Data;
    async fn send(connection: &Self::Connection, data: &Self::Data);
}
// The idea is to bind to a address, and then give back connections, a continuous loop.
// I guess the thing to think about is will the connection be maintained? or
// every time we receive a new packet, we get back a "new connection". Probably for UDP it will
// have to be a new connection because no state is maintained, but TCP will be different.
pub trait Transport: Protocol {
    fn listen(&self, address: Self::Address) -> Box<dyn Iterator<Item = Self::Connection>>;
}

#[async_trait]
impl<P: Protocol> Protocol for UDP<P> {
    type Address = (P::Address, u16);
    type Connection = UDPConnection<P>;
    type Data = Vec<u8>;
    // In connect I can de-struct the address, because i specified right above that
    // the address for UDP is a tuple
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

    // So at this layer I should receive a IP packet, and I need to extract from it, just the
    // data. The thing is that the packet then should be part of the UDP Connection? Also, this
    // should probably call the receive in the lower layers first...
    async fn receive(connection: &mut Self::Connection) -> Self::Data {
        unimplemented!()
    }

    async fn send(connection: &Self::Connection, data: &Self::Data) {
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
#[async_trait]
impl Protocol for Ether {
    type Address = MacAddr;
    type Connection = EtherConnection;
    type Data = Vec<u8>;

    fn connect(
        &self,
        source_address: Self::Address,
        destination_address: Self::Address,
    ) -> Self::Connection {
        EtherConnection {
            source_mac: source_address,
            destination_mac: destination_address,
            sender: self.sender.clone(),
            receiver: self.sender.subscribe(),
        }
    }

    async fn receive(connection: &mut Self::Connection) -> Self::Data {
        loop {
            let data = connection.receiver.recv().await.unwrap();
            let packet = pnet_packet::ethernet::EthernetPacket::new(&data).unwrap();
            let packet_source = packet.get_source();
            let packet_destination = packet.get_destination();
            if packet_source == connection.destination_mac
                && packet_destination == connection.source_mac
            {
                return packet.payload().to_vec();
            }
        }
    }

    async fn send(connection: &Self::Connection, data: &Self::Data) {}
}
#[async_trait]
impl<P: Protocol<Data = Vec<u8>>, F: Fn(Ipv4Addr) -> P::Address> Protocol for IP<P, F> {
    type Address = Ipv4Addr;
    type Connection = IPConnection<P>;
    type Data = Vec<u8>;

    // The source and destination address for the inner protocol will be the MAC address, and so
    // there type is not what is the type listed here.
    fn connect(
        &self,
        source_address: Self::Address,
        destination_address: Self::Address,
    ) -> Self::Connection {
        IPConnection {
            connection: self.inner_protocol.connect(
                (self.address_translator)(source_address),
                (self.address_translator)(destination_address),
            ),
            source_ip: source_address,
            destination_ip: destination_address,
        }
    }

    async fn receive(connection: &mut Self::Connection) -> Self::Data {
        let data = P::receive(&mut connection.connection).await;
        let packet = pnet_packet::ipv4::Ipv4Packet::new(&data).unwrap();
        packet.payload().to_vec()
    }

    async fn send(connection: &Self::Connection, data: &Self::Data) {
        todo!()
    }
}

pub struct IPConnection<P: Protocol> {
    connection: P::Connection,
    source_ip: Ipv4Addr,
    destination_ip: Ipv4Addr,
}

pub struct EtherConnection {
    source_mac: MacAddr,
    destination_mac: MacAddr,
    sender: tokio::sync::broadcast::Sender<Vec<u8>>,
    receiver: tokio::sync::broadcast::Receiver<Vec<u8>>,
}

pub struct Ether {
    sender: tokio::sync::broadcast::Sender<Vec<u8>>,
    receiver: tokio::sync::broadcast::Receiver<Vec<u8>>,
}

pub struct UDP<P> {
    inner_protocol: P,
}

pub struct IP<P: Protocol, F: Fn(Ipv4Addr) -> P::Address> {
    inner_protocol: P,
    address_translator: F,
}

pub trait Network: Protocol {}

pub trait Link: Protocol {}
