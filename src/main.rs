use async_trait::async_trait;
use pnet_base::MacAddr;
use pnet_packet::Packet;
use std::convert::TryInto;
use std::net::Ipv4Addr;
use tokio::sync::broadcast;
use tokio_stream::Stream;

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
    type ConnectionConfig;
    fn connect(
        &self,
        source_address: Self::Address,
        destination_address: Self::Address,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connection;
    async fn receive(connection: &mut Self::Connection) -> Self::Data;
    async fn send(connection: &Self::Connection, data: &Self::Data);
}
#[async_trait]
pub trait Bind: Protocol {
    type ServerConnection: Send + Sync;
    fn bind(address: Self::Address) -> Self::ServerConnection;
    async fn receive(connection: &mut Self::ServerConnection) -> Self::Data;
}

// The idea is to bind to a address, and then give back connections, a continuous loop.
// I guess the thing to think about is will the connection be maintained? or
// every time we receive a new packet, we get back a "new connection". Probably for UDP it will
// have to be a new connection because no state is maintained, but TCP will be different. THIS is
// for only new connections, normally the server will just use send/receive
pub trait Transport: Protocol {
    fn listen(&self, address: Self::Address) -> Box<dyn Stream<Item = Self::Connection>>;
}

#[async_trait]
impl<P: Protocol + SupportedConfiguration<Self>> Protocol for UDP<P> {
    type Address = (P::Address, u16);
    type Connection = UDPConnection<P>;
    type Data = Vec<u8>;
    type ConnectionConfig = ();

    // In connect I can de-struct the address, because i specified right above that
    // the address for UDP is a tuple
    fn connect(
        &self,
        (source_address, source_port): Self::Address,
        (destination_address, destination_port): Self::Address,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connection {
        UDPConnection {
            connection: self.inner_protocol.connect(
                source_address,
                destination_address,
                P::get_config(),
            ),
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

impl<P: Protocol + SupportedConfiguration<Self>> Transport for UDP<P> {
    fn listen(&self, address: Self::Address) -> Box<dyn Stream<Item = Self::Connection>> {
        todo!()
    }
}

pub struct UDPConnection<P: Protocol> {
    connection: P::Connection,
    source_port: u16,
    destination_port: u16,
}

pub struct EtherConnectionConfig {
    ether_type: pnet_packet::ethernet::EtherType,
}

pub trait SupportedConfiguration<P>: Protocol {
    fn get_config() -> Self::ConnectionConfig;
}

impl<P, F> SupportedConfiguration<IP<P, F>> for Ether {
    fn get_config() -> Self::ConnectionConfig {
        EtherConnectionConfig {
            ether_type: pnet_packet::ethernet::EtherTypes::Ipv4,
        }
    }
}

#[async_trait]
impl Protocol for Ether {
    type Address = MacAddr;
    type Connection = EtherConnection;
    type Data = Vec<u8>;
    type ConnectionConfig = EtherConnectionConfig;

    fn connect(
        &self,
        source_address: Self::Address,
        destination_address: Self::Address,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connection {
        EtherConnection {
            source_mac: source_address,
            destination_mac: destination_address,
            config: connection_config,
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
                && packet.get_ethertype() == connection.config.ether_type
            {
                return packet.payload().to_vec();
            }
        }
    }

    async fn send(connection: &Self::Connection, data: &Self::Data) {
        let packet_buffer = vec![0; 22 + data.len()];
        let mut packet =
            pnet_packet::ethernet::MutableEthernetPacket::owned(packet_buffer).unwrap();
        packet.set_source(connection.source_mac);
        packet.set_destination(connection.destination_mac);
        packet.set_ethertype(connection.config.ether_type);
        packet.set_payload(data);
        connection.sender.send(packet.packet().to_vec()).unwrap();
    }
}
#[async_trait]
impl<P: Protocol<Data = Vec<u8>> + SupportedConfiguration<Self>, F: Fn(Ipv4Addr) -> P::Address>
    Protocol for IP<P, F>
{
    type Address = Ipv4Addr;
    type Connection = IPConnection<P>;
    type Data = Vec<u8>;
    type ConnectionConfig = ();

    // The source and destination address for the inner protocol will be the MAC address, and so
    // there type is not what is the type listed here.
    fn connect(
        &self,
        source_address: Self::Address,
        destination_address: Self::Address,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connection {
        IPConnection {
            connection: self.inner_protocol.connect(
                (self.address_translator)(source_address),
                (self.address_translator)(destination_address),
                P::get_config(),
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
        // double check the packet setup. DOUBLE CHECK!!!!!!!!!!!!!!!!!!!!!!!!!!!
        let packet_buffer = vec![0; 20 + data.len()];
        let packet_total_len = packet_buffer.len();
        let mut packet = pnet_packet::ipv4::MutableIpv4Packet::owned(packet_buffer).unwrap();
        packet.set_version(4);
        packet.set_header_length(5);
        packet.set_dscp(0); // This field is largely unimportant
        packet.set_ecn(0);
        packet.set_total_length(packet_total_len as u16);
        packet.set_flags(0);
        packet.set_fragment_offset(0); // <--- This is not correct, reconsider
        packet.set_ttl(10);
        packet.set_next_level_protocol(pnet_packet::ip::IpNextHeaderProtocol::new(17));
        packet.set_checksum(0); // <--- Reconsider this also, maybe make full packet,then get
                                // underlying vector, make another packet?
        packet.set_source(connection.source_ip);
        packet.set_destination(connection.destination_ip);
        P::send(&connection.connection, &packet.packet().to_vec());
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
    config: EtherConnectionConfig,
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

pub struct IP<P, F> {
    inner_protocol: P,
    address_translator: F,
}

pub trait Network: Protocol {}

pub trait Link: Protocol {}
