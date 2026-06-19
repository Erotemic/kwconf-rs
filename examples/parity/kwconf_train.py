#!/usr/bin/env python3
"""Python kwconf version of the parity demo."""

import json
import kwconf


class TrainConfig(kwconf.Config):
    """Train a model."""

    lr = kwconf.Value(0.001, help='Learning rate.')
    mode = kwconf.Value('fast', choices=['fast', 'safe'], help='Run mode.')
    tags = kwconf.Value(default_factory=list, parser='csv', help='Comma-separated tags.')
    cache = kwconf.Value(True, help='Enable cache.')
    width = kwconf.Value(128, help='Image width.')


def main():
    cfg = TrainConfig.cli()
    print(json.dumps(dict(cfg), indent=2, sort_keys=True))


if __name__ == '__main__':
    main()
