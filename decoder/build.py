"""
Author: Samuel Lipsutz
Date: 2025
"""

import json
import os
import random as rnd
import rsa.common
import rsa.prime
import random

import typing
import rsa.transform
import subprocess
import prime_gen
import gen_subscription
# Get decoder ID environment variable
decoder_id = os.getenv("DECODER_ID")

# Generate a seed for each channel; you need a secret from secrets/secrets.json
secretsfile = open("secrets/secrets.json").read()
secrets = json.loads(secretsfile)
secret = secrets["systemsecret"]
channels = secrets["channels"]

def get_key_iv(seed) -> bytes:
    return random.Random(seed).randbytes(32)

keys = [get_key_iv((secret << 64) + (int(decoder_id, base=0) << 32) + channel) for channel in channels]
print("Keys generated")

# Export the keys to a file
open("src/keys.bin", "wb").write(b"".join(keys))

# Generate the channel 0 subscription
sub = gen_subscription.gen_subscription(secretsfile, decoder_id, 0, 2**64 - 1, 0)
open("src/emergency.bin", "wb").write(gen_subscription.gen_subscription(secretsfile, int(decoder_id, base=0), 0, 2**64 - 1, 0))
print("Emergency subscription generated")



# Set the CHANNELS env variable to the channels (other than 0) concatenated with commas
os.putenv("CHANNELS", ",".join([str(channel) for channel in channels if channel != 0]))

# Build the decoder
subprocess.run(["cargo", "build", "--release"], cwd="/decoder")

# Move the output to /out
subprocess.run(["mv", "target/release/decoder", "/out"], cwd="/decoder")