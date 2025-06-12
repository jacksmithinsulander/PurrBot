#!/usr/bin/env python3
"""
Test script for vsock communication with 9sdk-enclave
"""

import socket
import json
import struct
import sys
import argparse

def send_request(sock, request):
    """Send a length-prefixed JSON request"""
    request_bytes = json.dumps(request).encode()
    length = struct.pack('>I', len(request_bytes))
    sock.sendall(length + request_bytes)

def receive_response(sock):
    """Receive a length-prefixed JSON response"""
    # Read length
    length_bytes = sock.recv(4)
    if len(length_bytes) != 4:
        raise Exception("Failed to read response length")
    
    length = struct.unpack('>I', length_bytes)[0]
    
    # Read body
    body = b''
    while len(body) < length:
        chunk = sock.recv(length - len(body))
        if not chunk:
            raise Exception("Connection closed while reading response")
        body += chunk
    
    return json.loads(body)

def test_tcp_connection(host, port):
    """Test TCP connection to enclave"""
    print(f"Testing TCP connection to {host}:{port}")
    
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(5)
    
    try:
        sock.connect((host, port))
        print("✓ TCP connection successful")
        
        # Test SetupConfig
        print("\nTesting SetupConfig...")
        request = {"SetupConfig": {"password": "test_password"}}
        send_request(sock, request)
        response = receive_response(sock)
        
        if "ConfigSetup" in response:
            print(f"✓ SetupConfig successful")
            config = json.loads(response["ConfigSetup"]["config"])
            print(f"  - Password hash: {config['password_hash'][:20]}...")
            print(f"  - Salt1: {config['salt1'][:20]}...")
            print(f"  - Salt2: {config['salt2'][:20]}...")
        else:
            print(f"✗ SetupConfig failed: {response}")
            
        # Test VerifyAndDeriveKeys
        print("\nTesting VerifyAndDeriveKeys...")
        request = {"VerifyAndDeriveKeys": {"password": "test_password"}}
        send_request(sock, request)
        response = receive_response(sock)
        
        if "Keys" in response:
            print(f"✓ VerifyAndDeriveKeys successful")
            print(f"  - Key1 length: {len(response['Keys']['key1'])} bytes")
            print(f"  - Key2 length: {len(response['Keys']['key2'])} bytes")
        else:
            print(f"✗ VerifyAndDeriveKeys failed: {response}")
            
    except Exception as e:
        print(f"✗ TCP test failed: {e}")
    finally:
        sock.close()

def test_vsock_connection(cid, port):
    """Test vsock connection to enclave"""
    print(f"Testing vsock connection to CID {cid}, Port {port}")
    
    try:
        sock = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM)
        sock.settimeout(5)
    except AttributeError:
        print("✗ vsock not supported on this system")
        print("  Make sure you're running on a Linux system with vsock support")
        return
    
    try:
        sock.connect((cid, port))
        print("✓ Vsock connection successful")
        
        # Test SetupConfig
        print("\nTesting SetupConfig over vsock...")
        request = {"SetupConfig": {"password": "vsock_test_password"}}
        send_request(sock, request)
        response = receive_response(sock)
        
        if "ConfigSetup" in response:
            print(f"✓ SetupConfig successful")
            config = json.loads(response["ConfigSetup"]["config"])
            print(f"  - Password hash: {config['password_hash'][:20]}...")
        else:
            print(f"✗ SetupConfig failed: {response}")
            
    except Exception as e:
        print(f"✗ Vsock test failed: {e}")
    finally:
        sock.close()

def main():
    parser = argparse.ArgumentParser(description='Test 9sdk-enclave connectivity')
    parser.add_argument('--mode', choices=['tcp', 'vsock', 'both'], default='both',
                        help='Connection mode to test')
    parser.add_argument('--tcp-host', default='127.0.0.1',
                        help='TCP host address')
    parser.add_argument('--tcp-port', type=int, default=5005,
                        help='TCP port')
    parser.add_argument('--vsock-cid', type=int, default=16,
                        help='Vsock CID')
    parser.add_argument('--vsock-port', type=int, default=5005,
                        help='Vsock port')
    
    args = parser.parse_args()
    
    print("9SDK Enclave Connectivity Test")
    print("==============================\n")
    
    if args.mode in ['tcp', 'both']:
        test_tcp_connection(args.tcp_host, args.tcp_port)
        if args.mode == 'both':
            print("\n" + "="*50 + "\n")
    
    if args.mode in ['vsock', 'both']:
        test_vsock_connection(args.vsock_cid, args.vsock_port)
    
    print("\nTest completed!")

if __name__ == "__main__":
    main()