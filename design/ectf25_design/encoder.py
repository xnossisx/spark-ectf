"""
Author: Garrick Schinkel
Date: 2025



"""

import argparse
import struct
import json
import gmpy2
from sympy import isprime
import time
from blake3 import blake3
from Crypto.PublicKey import ECC
from Crypto.Signature import eddsa
from Crypto.Hash import SHA512 

# Hashes it with Blake3
def compress(n, section):
    compressed = int.from_bytes(blake3(section.to_bytes(1, byteorder="big")).update(n.to_bytes(128, byteorder="big")).digest()) & (2 ** 128 - 1)
    return compressed


def wind_encoder(root, target):
    result = root
    for section in range(64, -1, -1):
        mask = 1 << section
        if mask & target > 0:
            result = compress(result, section)
    return result

class Encoder:
    channel_cache = -1
    cache_mask = 0xfffffffffff00000 # This encoder caches 10 out of the 16 nibbles in a timestamp
    cached_timestamp = -1
    cached_forward = -1
    cached_backward = -1
    def __init__(self, secrets: bytes):
        """
        You **may not** change the arguments or returns of this function!

        :param secrets: Contents of the secrets file generated by
            ectf25_design.gen_secrets
        """
        # TODO: parse your secrets data here and run any necessary pre-processing to
        #   improve the throughput of Encoder.encode
        # Load the json of the secrets file
        secrets = json.loads(secrets)
        self.signer = ECC.import_key(encoded=secrets["private"], curve_name="Ed25519")

        # Load the example secrets for use in Encoder.encode
        # This will be "EXAMPLE" in the reference design"
        self.secrets = secrets

    def encode(self, channel: int, frame: bytes, timestamp: int) -> bytes:
    
        """The frame encoder function

        This will be called for every frame that needs to be encoded before being
        transmitted by the satellite to all listening TVs

        You **may not** change the arguments or returns of this function!

        :param channel: 16b unsigned channel number. Channel 0 is the emergency
            broadcast that must be decodable by all channels.
        :param frame: Frame to encode. Max frame size is 64 bytes.
        :param timestamp: 64b timestamp to use for encoding. **NOTE**: This value may
            have no relation to the current timestamp, so you should not compare it
            against the current time. The timestamp is guaranteed to strictly
            monotonically increase (always go up) with subsequent calls to encode

        :returns: The encoded frame, which will be sent to the Decoder
        """
        # TODO: encode the satellite frames so that they meet functional and
        #  security requirements

        end_of_time = 2**64 - 1

        if self.channel_cache != channel or (timestamp & self.cache_mask) != self.cached_timestamp:
            # Break the timestamp into two parts
            self.cached_timestamp = timestamp & self.cache_mask
            self.channel_cache = channel
            forward_root = self.secrets[str(channel)]["forward"]
            backward_root = self.secrets[str(channel)]["backward"]
            self.cached_forward = wind_encoder(forward_root, self.cached_timestamp)
            self.cached_backward = wind_encoder(backward_root, (end_of_time - self.cached_timestamp) & self.cache_mask)

        extra = timestamp & ~self.cache_mask

        forward = wind_encoder(self.cached_forward, extra)
        backward = wind_encoder(self.cached_backward, (end_of_time & ~self.cache_mask) - extra)

        guard = ((forward ^ backward) * (0x5CF481FFE6F11B408D66FFF23E5AB827B33DE52A2B3CECB41151001328ED091FBE600B23F21FBF327BB013A8267590805548377BAFDEBB6C467AF95F56AF3AE7)) % (2 ** 512)

        signature = eddsa.new(key=self.signer, mode='rfc8032', context=channel.to_bytes(4)).sign(SHA512.new(frame))

        return struct.pack("<IQ", channel, timestamp) + signature + (guard ^ int.from_bytes(frame)).to_bytes(64)


def main():
    """A test main to one-shot encode a frame

    This function is only for your convenience and will not be used in the final design.

    After pip-installing, you should be able to call this with:
        python3 -m ectf25_design.encoder path/to/test.secrets 1 "frame to encode" 100
    """
    parser = argparse.ArgumentParser(prog="ectf25_design.encoder")
    parser.add_argument(
        "secrets_file", type=argparse.FileType("rb"), help="Path to the secrets file"
    )
    parser.add_argument("channel", type=int, help="Channel to encode for")
    parser.add_argument("frame", help="Contents of the frame")
    parser.add_argument("timestamp", type=int, help="64b timestamp to use")
    parser.add_argument(
        "--time",
        "-t",
        action="store_true",
        help="Perform timing test",
    )
    args = parser.parse_args()

    encoder = Encoder(args.secrets_file.read())

    if args.time:
        s = time.time()
        for i in range(1000):
            encoder.encode(args.channel, args.frame.encode(), args.timestamp)
        d = time.time() - s
        print(d)
    else:   
        print(repr(encoder.encode(args.channel, args.frame.encode(), args.timestamp)))

if __name__ == "__main__":
    main()
