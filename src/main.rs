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

    let (tx, mut rx1) = broadcast::channel(16);

    let computer_1 = Ether {
        sender: tx,
        receiver: rx1,
    };
}
#[async_trait]
pub trait Protocol {
    type Address: Copy + Send + Sync;
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

// Currently failure is not built into the system, so like what happens if it fails to send etc
#[async_trait]
impl<P: Protocol<Data = Vec<u8>> + SupportedConfiguration<Self>> Protocol for UDP<P>
where
    Self: UDPChecksum<P>,
{
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
            destination_address,
            source_address,
        }
    }

    // So at this layer I should receive a IP packet, and I need to extract from it, just the
    // data. The thing is that the packet then should be part of the UDP Connection? Also, this
    // should probably call the receive in the lower layers first...
    async fn receive(connection: &mut Self::Connection) -> Self::Data {
        loop {
            let data = P::receive(&mut connection.connection).await;
            let packet = pnet_packet::udp::UdpPacket::new(&data).unwrap();
            if packet.get_source() == connection.destination_port
                && packet.get_destination() == connection.source_port
            {
                return packet.payload().to_vec();
            }
        }
    }

    async fn send(connection: &Self::Connection, data: &Self::Data) {
        let packet_buffer = vec![0; 8 + data.len()];
        let packet_total_len = packet_buffer.len();
        let mut packet = pnet_packet::udp::MutableUdpPacket::owned(packet_buffer).unwrap();
        packet.set_source(connection.source_port);
        packet.set_destination(connection.destination_port);
        packet.set_length(packet_total_len as u16);

        // I believe the issue here is that the checksum function requires a IPv4 address, and
        // the way the protocol is written, we cannot guarantee that...so either I can fake the
        // checksum or....
        let checksum = Self::calculate_checksum(
            packet.to_immutable(),
            connection.source_address,
            connection.destination_address,
        );
        packet.set_checksum(checksum);
        let packet_vec = packet.packet().to_vec();
        P::send(&connection.connection, &packet_vec).await;
    }
}

pub trait UDPChecksum<P: Protocol> {
    fn calculate_checksum(
        packet: pnet_packet::udp::UdpPacket,
        source_address: P::Address,
        destination_address: P::Address,
    ) -> u16;
}

impl<
        P: Protocol<Data = Vec<u8>> + SupportedConfiguration<IP<P, F>>,
        F: Fn(Ipv4Addr) -> P::Address,
        T,
    > UDPChecksum<IP<P, F>> for UDP<T>
{
    fn calculate_checksum(
        packet: pnet_packet::udp::UdpPacket,
        source_address: <IP<P, F> as Protocol>::Address,
        destination_address: <IP<P, F> as Protocol>::Address,
    ) -> u16 {
        pnet_packet::udp::ipv4_checksum(&packet, &source_address, &destination_address)
    }
}

impl<P: Protocol<Data = Vec<u8>> + SupportedConfiguration<Self>> Transport for UDP<P>
where
    Self: UDPChecksum<P>,
{
    fn listen(&self, address: Self::Address) -> Box<dyn Stream<Item = Self::Connection>> {
        todo!()
    }
}

pub struct UDPConnection<P: Protocol> {
    connection: P::Connection,
    source_port: u16,
    destination_port: u16,
    destination_address: P::Address,
    source_address: P::Address,
}

pub struct EtherConnectionConfig {
    ether_type: pnet_packet::ethernet::EtherType,
}

pub struct IPConnectionConfig {
    protocol: pnet_packet::ip::IpNextHeaderProtocol,
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

impl<
        P: Protocol<Data = Vec<u8>> + SupportedConfiguration<Self>,
        F: Fn(Ipv4Addr) -> P::Address,
        T,
    > SupportedConfiguration<UDP<T>> for IP<P, F>
{
    fn get_config() -> Self::ConnectionConfig {
        IPConnectionConfig {
            protocol: pnet_packet::ip::IpNextHeaderProtocols::Udp,
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
    type ConnectionConfig = IPConnectionConfig;

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
            config: connection_config,
        }
    }

    async fn receive(connection: &mut Self::Connection) -> Self::Data {
        loop {
            let data = P::receive(&mut connection.connection).await;
            let packet = pnet_packet::ipv4::Ipv4Packet::new(&data).unwrap();
            if packet.get_source() == connection.destination_ip
                && packet.get_destination() == connection.source_ip
                && connection.config.protocol == packet.get_next_level_protocol()
            {
                return packet.payload().to_vec();
            }
        }
    }

    async fn send(connection: &Self::Connection, data: &Self::Data) {
        let packet_buffer = vec![0; 20 + data.len()];
        let packet_total_len = packet_buffer.len();
        let mut packet = pnet_packet::ipv4::MutableIpv4Packet::owned(packet_buffer).unwrap();
        packet.set_version(4);
        packet.set_header_length(5);
        packet.set_total_length(packet_total_len as u16);
        packet.set_next_level_protocol(connection.config.protocol);
        packet.set_source(connection.source_ip);
        packet.set_destination(connection.destination_ip);
        let checksum = pnet_packet::ipv4::checksum(&packet.to_immutable());
        packet.set_checksum(checksum);
        let packet_vec = packet.packet().to_vec();
        P::send(&connection.connection, &packet_vec).await;
    }
}

pub struct IPConnection<P: Protocol> {
    connection: P::Connection,
    source_ip: Ipv4Addr,
    destination_ip: Ipv4Addr,
    config: IPConnectionConfig,
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
    address_translator: F, // THIS NEEDS TO BE IMPLEMENTED
                           //maybe have a ARP table like thing, where when you start uo you register ourself, with mac +
                           // Ip and when you finish you delete entry, and then this function can look it up
}

pub trait Network: Protocol {}

pub trait Link: Protocol {}
