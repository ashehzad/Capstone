use async_trait::async_trait;
use pnet_base::MacAddr;
use pnet_packet::Packet;
use std::fmt::Debug;
use std::net::Ipv4Addr;
use tokio::sync::broadcast;

// The main function is used to test the overall program. It is used to create the different
// levels of the protocol, and then send/receive data.
#[tokio::main]
async fn main() {
    let (tx, rx1) = broadcast::channel(16);

    let ether_1 = Ether {
        sender: tx.clone(),
        receiver: rx1,
    };

    let ip_1 = IP {
        inner_protocol: ether_1,
        address_translator: |ipv4_address: Ipv4Addr| match ipv4_address.octets() {
            [192, 16, 16, 16] => MacAddr::new(10, 12, 14, 16, 18, 20),
            [190, 16, 16, 16] => MacAddr::new(12, 12, 14, 16, 18, 20),
            _ => panic!("Unknown Address!!!!!!!!!!!!!!!!!"),
        },
    };

    let tcp_1 = TCP {
        inner_protocol: ip_1,
    };

    let ether_2 = Ether {
        receiver: tx.subscribe(),
        sender: tx,
    };

    let ip_2 = IP {
        inner_protocol: ether_2,
        address_translator: |ipv4_address: Ipv4Addr| match ipv4_address.octets() {
            [192, 16, 16, 16] => MacAddr::new(10, 12, 14, 16, 18, 20),
            [190, 16, 16, 16] => MacAddr::new(12, 12, 14, 16, 18, 20),
            _ => panic!("Unknown Address!!!!!!!!!!!!!!!!!"),
        },
    };

    let tcp_2 = TCP {
        inner_protocol: ip_2,
    };
    let mut server_connection = tcp_2.bind((Ipv4Addr::new(190, 16, 16, 16), 80));

    let connection_1_future = tcp_1.connect(
        (Ipv4Addr::new(192, 16, 16, 16), 80),
        (Ipv4Addr::new(190, 16, 16, 16), 80),
        (),
    );
    // let mut data = vec![1, 2, 4];
    // TCP::send(&connection_1, &data).await;
    let connection_2_future = tcp_2.next(&mut server_connection);

    let (connection_1, connection_2) = tokio::join!(connection_1_future, connection_2_future);

    println!("MADE CONNECTION");

    // let mut buffer = Vec::new();
    // UDP::receive(&mut connection_2, &mut buffer).await;
    // println!("{:?}", buffer);
    //
    // let mut client_buffer = Vec::new();
    // let server_data = vec![2, 5];
    // UDP::send(&connection_2, &server_data).await;
    // UDP::receive(&mut connection_1, &mut client_buffer).await;
    //
    // println!("{:?}", client_buffer);
    //
    // data = vec![1, 3, 4, 6];
    // UDP::send(&connection_1, &data).await;
    // UDP::receive(&mut connection_2, &mut buffer).await;
    // println!("{:?}", buffer);
}

// The code is structured around traits. First Bind, then Listener, and finally Protocol. It can
// be further subdivided based on the networking layer(ip, etc)

// BIND TRAIT SECTION
//++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++

// This protocol is used by the server to bind to a address and then listen/wait for any
// connections to that address. Once this is done, the server can then use that connection to
// communicate with the other address.
#[async_trait]
pub trait Bind: Protocol {
    // A server connection is created and bound to a address and used to listen for new connections
    type ServerConnection: Send + Sync;
    // bind is used to bind to a specific address and give back a server connection,which is then
    // used to get new connections
    fn bind(&self, address: Self::Address) -> Self::ServerConnection;
    // Next is used to listen on the server connection and when new connections are found, this
    // function returns them to be used.
    async fn next(&self, connection: &mut Self::ServerConnection) -> Self::Connection;
}

//*****UDP*****
#[async_trait]
impl<P: Listener + SupportedConfiguration<Self>> Bind for UDP<P>
where
    Self: UDPChecksum<P>,
{
    type ServerConnection = UDPServerConnection<P>;

    fn bind(&self, (address, port): Self::Address) -> Self::ServerConnection {
        UDPServerConnection {
            source_port: port,
            source_address: address,
            inner_connection: self.inner_protocol.listen(address, P::get_config()),
        }
    }

    async fn next(&self, connection: &mut Self::ServerConnection) -> Self::Connection {
        loop {
            //if check set
            let mut buffer = Vec::new();
            let sender_address = P::next(&mut connection.inner_connection, &mut buffer).await;
            let packet = pnet_packet::udp::UdpPacket::new(&buffer).unwrap();
            if packet.get_destination() == connection.source_port {
                return UDPConnection {
                    connection: self
                        .inner_protocol
                        .connect(connection.source_address, sender_address, P::get_config())
                        .await,
                    cached_payload: Some(packet.payload().to_vec()),
                    source_port: connection.source_port,
                    destination_port: packet.get_source(),
                    destination_address: sender_address,
                    source_address: connection.source_address,
                };
            }
        }
    }
}

// Struct to hold a Server Connection for UDP
pub struct UDPServerConnection<P: Listener> {
    source_port: u16,
    source_address: P::Address,
    inner_connection: P::ListenConnection,
}

//*****TCP*****
#[async_trait]
impl<P: Listener + SupportedConfiguration<Self>> Bind for TCP<P>
where
    Self: TCPChecksum<P>,
{
    type ServerConnection = TCPServerConnection<P>;

    fn bind(&self, (source_address, source_port): Self::Address) -> Self::ServerConnection {
        TCPServerConnection {
            source_port,
            source_address,
            inner_connection: self.inner_protocol.listen(source_address, P::get_config()),
        }
    }

    async fn next(&self, connection: &mut Self::ServerConnection) -> Self::Connection {
        let mut packet_buffer = vec![0; 20];
        loop {
            packet_buffer.clear();
            let sender_address =
                P::next(&mut connection.inner_connection, &mut packet_buffer).await;
            println!("{:?}", packet_buffer);
            let syn_packet = pnet_packet::tcp::TcpPacket::new(&packet_buffer).unwrap();

            let is_syn = syn_packet.get_flags() & 0b10 != 0;
            let destination_port = syn_packet.get_destination();
            let sender_seq_number = syn_packet.get_sequence();
            let sender_port = syn_packet.get_source();
            println!("{:#?}", syn_packet);
            if !is_syn || destination_port != connection.source_port {
                continue;
            }
            packet_buffer.fill(0);
            let mut syn_ack_packet =
                pnet_packet::tcp::MutableTcpPacket::new(&mut packet_buffer).unwrap();
            syn_ack_packet.set_source(connection.source_port);
            syn_ack_packet.set_destination(sender_port);
            syn_ack_packet.set_acknowledgement(sender_seq_number + 1);
            syn_ack_packet.set_flags(0b10010);
            syn_ack_packet.set_sequence(0);
            let mut ip_connection = self
                .inner_protocol
                .connect(connection.source_address, sender_address, P::get_config())
                .await;

            P::send(&ip_connection, syn_ack_packet.packet()).await;
            packet_buffer.clear();
            P::receive(&mut ip_connection, &mut packet_buffer).await;

            let ack_packet = pnet_packet::tcp::TcpPacket::new(&packet_buffer).unwrap();

            let destination_port = ack_packet.get_destination();
            let ack_number = ack_packet.get_acknowledgement();
            let is_ack = ack_packet.get_flags() & 0b10000 != 0;

            if !is_ack || destination_port != connection.source_port || ack_number != 1 {
                continue;
            }

            return TCPConnection {
                connection: ip_connection,
                source_port: connection.source_port,
                destination_port: sender_port,
                destination_address: sender_address,
                source_address: connection.source_address,
            };
        }
    }
}

// Struct to hold a Server Connection for TCP
pub struct TCPServerConnection<P: Listener> {
    source_port: u16,
    source_address: P::Address,
    inner_connection: P::ListenConnection,
}
// LISTENER TRAIT SECTION
//+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
// Trait used for the lower protocols on the server side. To establish a connection, the server
// side needs to get the addresses for the lower protocols. This trait is used to get those
// addresses so that the transport layer can establish a connection
#[async_trait]
pub trait Listener: Protocol {
    type ListenConnection: Send + Sync;
    fn listen(
        &self,
        address: Self::Address,
        config: Self::ConnectionConfig,
    ) -> Self::ListenConnection;
    async fn next(connection: &mut Self::ListenConnection, buffer: &mut Vec<u8>) -> Self::Address;
}

//*****IP*****
#[async_trait]
impl<P: Listener + SupportedConfiguration<Self>, F: Fn(Ipv4Addr) -> P::Address + Send + Sync>
    Listener for IP<P, F>
{
    type ListenConnection = IPListenConnection<P>;

    fn listen(
        &self,
        address: Self::Address,
        config: Self::ConnectionConfig,
    ) -> Self::ListenConnection {
        IPListenConnection {
            source_address: address,
            config,
            inner_connection: self
                .inner_protocol
                .listen((self.address_translator)(address), P::get_config()),
        }
    }

    async fn next(connection: &mut Self::ListenConnection, buffer: &mut Vec<u8>) -> Self::Address {
        loop {
            let mut underlying_data = Vec::new();
            P::next(&mut connection.inner_connection, &mut underlying_data).await;
            let packet = pnet_packet::ipv4::Ipv4Packet::new(&underlying_data).unwrap();
            if packet.get_destination() == connection.source_address
                && connection.config.protocol == packet.get_next_level_protocol()
            {
                buffer.extend_from_slice(packet.payload());
                return packet.get_source();
            }
        }
    }
}

pub struct IPListenConnection<P: Listener> {
    source_address: Ipv4Addr,
    config: IPConnectionConfig,
    inner_connection: P::ListenConnection,
}

//*****ETHER*****
#[async_trait]
impl Listener for Ether {
    type ListenConnection = EtherListenConnection;

    fn listen(
        &self,
        address: Self::Address,
        config: Self::ConnectionConfig,
    ) -> Self::ListenConnection {
        EtherListenConnection {
            source_address: address,
            config,
            receiver: self.sender.subscribe(),
        }
    }

    async fn next(connection: &mut Self::ListenConnection, buffer: &mut Vec<u8>) -> Self::Address {
        loop {
            let data = connection.receiver.recv().await.unwrap();
            let packet = pnet_packet::ethernet::EthernetPacket::new(&data).unwrap();
            let packet_source = packet.get_source();
            let packet_destination = packet.get_destination();
            if packet_destination == connection.source_address
                && packet.get_ethertype() == connection.config.ether_type
            {
                buffer.extend_from_slice(packet.payload());
                return packet_source;
            }
        }
    }
}

pub struct EtherListenConnection {
    source_address: MacAddr,
    config: EtherConnectionConfig,
    receiver: tokio::sync::broadcast::Receiver<Vec<u8>>,
}

// PROTOCOL TRAIT SECTION
//++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
// The protocol trait is implemented by all levels of the networking stack. It contains methods
// to connect with a remote address, and then using that connection send and receive data.
#[async_trait]
pub trait Protocol: Send + Sync {
    type Address: Copy + Send + Sync;
    type Connection: Send + Sync;
    type ConnectionConfig: Send;

    async fn connect(
        &self,
        source_address: Self::Address,
        destination_address: Self::Address,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connection;
    async fn receive(connection: &mut Self::Connection, buffer: &mut Vec<u8>);
    async fn send(connection: &Self::Connection, data: &[u8]);
}

// Currently failure is not built into the system, so like what happens if it fails to send etc
// So, the thing with UDP is that ut us really meant to send individual packets, and not streams
// of data, so we only need to worry about a packet at a time.

//*****UDP*****
#[async_trait]
impl<P: Protocol + SupportedConfiguration<Self>> Protocol for UDP<P>
where
    Self: UDPChecksum<P>,
{
    type Address = (P::Address, u16);
    type Connection = UDPConnection<P>;
    type ConnectionConfig = ();

    async fn connect(
        &self,
        (source_address, source_port): Self::Address,
        (destination_address, destination_port): Self::Address,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connection {
        UDPConnection {
            connection: self
                .inner_protocol
                .connect(source_address, destination_address, P::get_config())
                .await,
            source_port,
            cached_payload: None,
            destination_port,
            destination_address,
            source_address,
        }
    }

    async fn receive(connection: &mut Self::Connection, buffer: &mut Vec<u8>) {
        if let Some(cached_payload) = connection.cached_payload.take() {
            buffer.extend(cached_payload);
            return;
        }
        loop {
            let mut underlying_buffer = Vec::new();
            P::receive(&mut connection.connection, &mut underlying_buffer).await;
            let packet = pnet_packet::udp::UdpPacket::new(&underlying_buffer).unwrap();
            println!("{:?}", packet);
            if packet.get_source() == connection.destination_port
                && packet.get_destination() == connection.source_port
            {
                buffer.append(&mut packet.payload().to_vec());
                return;
            }
        }
    }

    async fn send(connection: &Self::Connection, data: &[u8]) {
        let packet_buffer = vec![0; 8 + data.len()];
        let packet_total_len = packet_buffer.len();
        let mut packet = pnet_packet::udp::MutableUdpPacket::owned(packet_buffer).unwrap();
        packet.set_source(connection.source_port);
        packet.set_destination(connection.destination_port);
        packet.set_length(packet_total_len as u16);
        packet.set_payload(data);
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
        P: Protocol + SupportedConfiguration<IP<P, F>>,
        F: Fn(Ipv4Addr) -> P::Address + Send + Sync,
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

pub struct UDPConnection<P: Protocol> {
    cached_payload: Option<Vec<u8>>,
    connection: P::Connection,
    source_port: u16,
    destination_port: u16,
    destination_address: P::Address,
    source_address: P::Address,
}

pub struct UDP<P> {
    inner_protocol: P,
}

//*****TCP*****
//-----------------------------------------------------------------------------------------------

#[async_trait]
impl<P: Protocol + SupportedConfiguration<Self>> Protocol for TCP<P>
where
    Self: TCPChecksum<P>,
{
    type Address = (P::Address, u16);
    type Connection = TCPConnection<P>;
    type ConnectionConfig = ();

    async fn connect(
        &self,
        (source_address, source_port): Self::Address,
        (destination_address, destination_port): Self::Address,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connection {
        let mut ip_connection = self
            .inner_protocol
            .connect(source_address, destination_address, P::get_config())
            .await;
        let mut packet_buffer = vec![0; 20];
        let mut syn_packet = pnet_packet::tcp::MutableTcpPacket::new(&mut packet_buffer).unwrap();
        syn_packet.set_source(source_port);
        syn_packet.set_destination(destination_port);
        syn_packet.set_flags(2);
        syn_packet.set_sequence(0);
        println!("{:#?}", syn_packet);
        P::send(&ip_connection, syn_packet.packet()).await;
        packet_buffer.clear();
        P::receive(&mut ip_connection, &mut packet_buffer).await;
        let syn_ack_packet = pnet_packet::tcp::TcpPacket::new(&packet_buffer).unwrap();
        let flags = syn_ack_packet.get_flags();
        let is_ack = flags & 0b10 != 0;
        let is_syn = flags & 0b10000 != 0;
        let server_seq = syn_ack_packet.get_sequence();

        if !is_ack || !is_syn {
            panic!()
        }
        packet_buffer.fill(0);
        let mut ack_packet = pnet_packet::tcp::MutableTcpPacket::new(&mut packet_buffer).unwrap();
        ack_packet.set_source(source_port);
        ack_packet.set_destination(destination_port);
        ack_packet.set_flags(1 << 4);
        ack_packet.set_acknowledgement(server_seq + 1);
        P::send(&ip_connection, ack_packet.packet()).await;
        TCPConnection {
            connection: ip_connection,
            source_port,
            destination_port,
            destination_address,
            source_address,
        }
    }

    async fn receive(connection: &mut Self::Connection, buffer: &mut Vec<u8>) {
        todo!()
    }

    async fn send(connection: &Self::Connection, data: &[u8]) {
        todo!()
    }
}

pub trait TCPChecksum<P: Protocol> {
    fn calculate_checksum(
        packet: pnet_packet::tcp::TcpPacket,
        source_address: P::Address,
        destination_address: P::Address,
    ) -> u16;
}

impl<
        P: Protocol + SupportedConfiguration<IP<P, F>>,
        F: Fn(Ipv4Addr) -> P::Address + Send + Sync,
        T,
    > TCPChecksum<IP<P, F>> for TCP<T>
{
    fn calculate_checksum(
        packet: pnet_packet::tcp::TcpPacket,
        source_address: <IP<P, F> as Protocol>::Address,
        destination_address: <IP<P, F> as Protocol>::Address,
    ) -> u16 {
        pnet_packet::tcp::ipv4_checksum(&packet, &source_address, &destination_address)
    }
}

pub struct TCPConnection<P: Protocol> {
    connection: P::Connection,
    source_port: u16,
    destination_port: u16,
    destination_address: P::Address,
    source_address: P::Address,
}

pub struct TCP<P> {
    inner_protocol: P,
}

//*****IP*****
//-----------------------------------------------------------------------------------------------

#[async_trait]
impl<P: Protocol + SupportedConfiguration<Self>, F: Fn(Ipv4Addr) -> P::Address + Send + Sync>
    Protocol for IP<P, F>
{
    type Address = Ipv4Addr;
    type Connection = IPConnection<P>;
    type ConnectionConfig = IPConnectionConfig;

    async fn connect(
        &self,
        source_address: Self::Address,
        destination_address: Self::Address,
        connection_config: Self::ConnectionConfig,
    ) -> Self::Connection {
        IPConnection {
            connection: self
                .inner_protocol
                .connect(
                    (self.address_translator)(source_address),
                    (self.address_translator)(destination_address),
                    P::get_config(),
                )
                .await,
            source_ip: source_address,
            destination_ip: destination_address,
            config: connection_config,
        }
    }

    async fn receive(connection: &mut Self::Connection, buffer: &mut Vec<u8>) {
        loop {
            let mut underlying_buffer = Vec::new();
            P::receive(&mut connection.connection, &mut underlying_buffer).await;
            let packet = pnet_packet::ipv4::Ipv4Packet::new(&underlying_buffer).unwrap();
            if packet.get_source() == connection.destination_ip
                && packet.get_destination() == connection.source_ip
                && connection.config.protocol == packet.get_next_level_protocol()
            {
                return buffer.append(&mut packet.payload().to_vec());
            }
        }
    }

    async fn send(connection: &Self::Connection, data: &[u8]) {
        let packet_buffer = vec![0; 20 + data.len()];
        println!("{:?}", data);
        let packet_total_len = packet_buffer.len();
        let mut packet = pnet_packet::ipv4::MutableIpv4Packet::owned(packet_buffer).unwrap();
        packet.set_version(4);
        packet.set_header_length(5);
        packet.set_total_length(packet_total_len as u16);
        packet.set_next_level_protocol(connection.config.protocol);
        packet.set_source(connection.source_ip);
        packet.set_destination(connection.destination_ip);
        packet.set_payload(data);
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

#[derive(Debug)]
pub struct IP<P, F> {
    inner_protocol: P,
    address_translator: F,
}

#[derive(Debug)]
pub struct IPConnectionConfig {
    protocol: pnet_packet::ip::IpNextHeaderProtocol,
}

// Trait used to get the underlying connection configuration(Ether, Ipv4 etc)
pub trait SupportedConfiguration<P>: Protocol {
    fn get_config() -> Self::ConnectionConfig;
}

//*****ETHER*****
//-------------------------------------------------------------------------------------------------
#[async_trait]
impl Protocol for Ether {
    type Address = MacAddr;
    type Connection = EtherConnection;
    type ConnectionConfig = EtherConnectionConfig;

    async fn connect(
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

    async fn receive(connection: &mut Self::Connection, buffer: &mut Vec<u8>) {
        loop {
            let data = connection.receiver.recv().await.unwrap();
            let packet = pnet_packet::ethernet::EthernetPacket::new(&data).unwrap();
            let packet_source = packet.get_source();
            let packet_destination = packet.get_destination();
            if packet_source == connection.destination_mac
                && packet_destination == connection.source_mac
                && packet.get_ethertype() == connection.config.ether_type
            {
                buffer.extend_from_slice(packet.payload());
                return;
            }
            println!("Ether");
        }
    }

    async fn send(connection: &Self::Connection, data: &[u8]) {
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

impl<P, F> SupportedConfiguration<IP<P, F>> for Ether {
    fn get_config() -> Self::ConnectionConfig {
        EtherConnectionConfig {
            ether_type: pnet_packet::ethernet::EtherTypes::Ipv4,
        }
    }
}

#[derive(Debug)]
pub struct EtherConnectionConfig {
    ether_type: pnet_packet::ethernet::EtherType,
}

#[derive(Debug)]
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

impl<
        P: Protocol + SupportedConfiguration<Self>,
        F: Fn(Ipv4Addr) -> P::Address + Send + Sync,
        T,
    > SupportedConfiguration<UDP<T>> for IP<P, F>
{
    fn get_config() -> Self::ConnectionConfig {
        IPConnectionConfig {
            protocol: pnet_packet::ip::IpNextHeaderProtocols::Udp,
        }
    }
}

impl<
        P: Protocol + SupportedConfiguration<Self>,
        F: Fn(Ipv4Addr) -> P::Address + Send + Sync,
        T,
    > SupportedConfiguration<TCP<T>> for IP<P, F>
{
    fn get_config() -> Self::ConnectionConfig {
        IPConnectionConfig {
            protocol: pnet_packet::ip::IpNextHeaderProtocols::Tcp,
        }
    }
}
