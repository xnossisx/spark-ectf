"""
Author: Samuel Lipsutz
Date: 2025
"""

import json
import os
import random as rnd
import random

import rsa.transform
import subprocess
import gen_subscription

from Crypto.PublicKey import ECC
from Crypto.Signature import eddsa
from Crypto.Hash import SHA512

# Get decoder ID environment variable
decoder_id = int(os.getenv("DECODER_ID"), base=0)

# Generate a seed for each channel; you need a secret from secrets/secrets.json
secretsfile = open("/secrets/secrets.json").read()
secrets = json.loads(secretsfile)
secret = secrets["systemsecret"]
channels = secrets["channels"]

def get_key_iv(seed) -> bytes:
    return random.Random(seed).randbytes(32)

keys = [get_key_iv((secret << 64) + (decoder_id << 32) + channel) for channel in channels]
print("Keys generated")

# Export the keys to a file
os.remove("src/keys.bin")
open("src/keys.bin", "xb+").write(b"".join(keys))

# Generate the channel 0 subscription
sub = gen_subscription.gen_subscription(secretsfile, decoder_id, 0, 2**64 - 1, 0)
os.remove("src/emergency.bin")
open("src/emergency.bin", "xb+").write(gen_subscription.gen_subscription(secretsfile, decoder_id, 0, 2**64 - 1, 0))
print("Emergency subscription generated")

# Export public ECC key

curve = ECC.import_key(encoded=secrets["public"], curve_name="Ed25519")
os.remove("src/public.bin")
with open("src/public.bin", "xb+") as f:
    # Dump the secrets to the file
    f.write(curve.public_key().export_key(format='raw'))


# Set the CHANNELS env variable to the channels (other than 0) concatenated with commas
os.putenv("CHANNELS", ",".join([str(channel) for channel in channels if channel != 0]))

# Build the decoder
subprocess.run(["cargo", "build", "--release"], cwd=".")

# Move the output to /out
# subprocess.run(["find", ".", "-name", "spark-decoder"])
subprocess.run(["mv", "target/thumbv7em-none-eabihf/release/spark-decoder", "/out"], cwd=".")