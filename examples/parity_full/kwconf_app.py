#!/usr/bin/env python3
"""Full Python kwconf parity app used by the kwconf-rs side-by-side demo.

This example intentionally uses the same config field names and shared YAML
fixtures as ``crates/kwconf/examples/kwconf_rs_full_app.rs``. It demonstrates
modal commands, nested subconfigs, aliases, choices, flags, csv/yaml string
parsers, config files, and argv overrides.
"""

from __future__ import annotations

import json
import sys

import kwconf


class DatasetConfig(kwconf.Config):
    root: str = kwconf.Value('data/images', help='Dataset root directory.')
    split: str = kwconf.Value('train', choices=['train', 'val', 'test'], help='Dataset split.')
    channels: list = kwconf.Value(default_factory=lambda: ['red', 'green', 'blue'], parser='csv', help='Channel names.')
    cache = kwconf.Flag(False, help='Cache decoded samples.')


class ModelConfig(kwconf.Config):
    arch: str = kwconf.Value('resnet', choices=['resnet', 'unet'], help='Model architecture.')
    depth: int = kwconf.Value(50, help='Model depth.')
    pretrained = kwconf.Flag(True, help='Use pretrained weights.')


class OptimizerConfig(kwconf.Config):
    lr: float = kwconf.Value(0.001, help='Learning rate.')
    weight_decay: float = kwconf.Value(0.0, help='Weight decay.')
    scheduler: str = kwconf.Value('none', choices=['none', 'cosine', 'step'], help='Scheduler kind.')


class LoggingConfig(kwconf.Config):
    level: str = kwconf.Value('INFO', choices=['DEBUG', 'INFO', 'WARNING'], help='Log level.')
    tags: list = kwconf.Value(default_factory=list, parser='csv', help='Run tags.')
    metadata: dict = kwconf.Value(default_factory=dict, parser='yaml', help='Free-form YAML metadata.')


class TrainConfig(kwconf.Config):
    __special_options__ = True
    __description__ = 'Train with nested configs and parser-aware string inputs.'

    profile: str = kwconf.Value('local', alias=['preset'], choices=['local', 'debug', 'cluster'], help='Execution profile.')
    dataset = kwconf.SubConfig(DatasetConfig)
    model = kwconf.SubConfig(ModelConfig)
    optimizer = kwconf.SubConfig(OptimizerConfig)
    logging = kwconf.SubConfig(LoggingConfig)
    dry_run = kwconf.Flag(False, alias=['dry'], help='Print the plan without running.')


class EvalConfig(kwconf.Config):
    __special_options__ = True
    __description__ = 'Evaluate a checkpoint with nested dataset settings.'

    checkpoint: str = kwconf.Value('runs/latest/checkpoint.pt', help='Checkpoint path.')
    dataset = kwconf.SubConfig(DatasetConfig)
    metrics: list = kwconf.Value(default_factory=lambda: ['accuracy'], parser='csv', help='Metric names.')
    threshold: float = kwconf.Value(0.5, help='Decision threshold.')
    report = kwconf.Flag(True, help='Write an evaluation report.')


class ExportConfig(kwconf.Config):
    __special_options__ = True
    __description__ = 'Export a checkpoint to an interchange format.'

    checkpoint: str = kwconf.Value('runs/latest/checkpoint.pt', help='Checkpoint path.')
    output: str = kwconf.Value('exported/model.onnx', help='Output path.')
    format: str = kwconf.Value('onnx', choices=['onnx', 'torchscript'], help='Export format.')
    opset: int = kwconf.Value(17, help='ONNX opset.')
    simplify = kwconf.Flag(True, help='Simplify the exported graph.')


def dataset_dict(cfg: DatasetConfig) -> dict:
    return {
        'cache': cfg.cache,
        'channels': list(cfg.channels),
        'root': cfg.root,
        'split': cfg.split,
    }


def train_dict(cfg: TrainConfig) -> dict:
    return {
        'dataset': dataset_dict(cfg.dataset),
        'dry_run': cfg.dry_run,
        'logging': {
            'level': cfg.logging.level,
            'metadata': dict(cfg.logging.metadata),
            'tags': list(cfg.logging.tags),
        },
        'model': {
            'arch': cfg.model.arch,
            'depth': cfg.model.depth,
            'pretrained': cfg.model.pretrained,
        },
        'optimizer': {
            'lr': cfg.optimizer.lr,
            'scheduler': cfg.optimizer.scheduler,
            'weight_decay': cfg.optimizer.weight_decay,
        },
        'profile': cfg.profile,
    }


def eval_dict(cfg: EvalConfig) -> dict:
    return {
        'checkpoint': cfg.checkpoint,
        'dataset': dataset_dict(cfg.dataset),
        'metrics': list(cfg.metrics),
        'report': cfg.report,
        'threshold': cfg.threshold,
    }


def export_dict(cfg: ExportConfig) -> dict:
    return {
        'checkpoint': cfg.checkpoint,
        'format': cfg.format,
        'opset': cfg.opset,
        'output': cfg.output,
        'simplify': cfg.simplify,
    }


def emit(command: str, config: dict) -> None:
    payload = {
        'command': command,
        'config': config,
    }
    print(json.dumps(payload, indent=2, sort_keys=True))


class TrainCommand(TrainConfig):
    @classmethod
    def main(cls, argv=None, **kwargs):
        kwargs = {k: v for k, v in kwargs.items() if v is not None}
        cfg = cls.cli(argv=argv, data=kwargs, allow_subconfig_overrides=True, special_options=True)
        emit('train', train_dict(cfg))
        return cfg


class EvalCommand(EvalConfig):
    @classmethod
    def main(cls, argv=None, **kwargs):
        kwargs = {k: v for k, v in kwargs.items() if v is not None}
        cfg = cls.cli(argv=argv, data=kwargs, allow_subconfig_overrides=True, special_options=True)
        emit('eval', eval_dict(cfg))
        return cfg


class ExportCommand(ExportConfig):
    @classmethod
    def main(cls, argv=None, **kwargs):
        kwargs = {k: v for k, v in kwargs.items() if v is not None}
        cfg = cls.cli(argv=argv, data=kwargs, special_options=True)
        emit('export', export_dict(cfg))
        return cfg


class KwconfParityApp(kwconf.ModalCLI):
    """Full kwconf parity demo with modal commands and nested configs."""

    __version__ = '0.1.0'

    train = kwconf.ModalValue(TrainCommand, alias=['fit'])
    eval = kwconf.ModalValue(EvalCommand, alias=['score'])
    export = ExportCommand


COMMANDS = {
    'train': ('train', TrainCommand),
    'fit': ('train', TrainCommand),
    'eval': ('eval', EvalCommand),
    'score': ('eval', EvalCommand),
    'export': ('export', ExportCommand),
}


def main(argv=None):
    if argv is None:
        argv = sys.argv[1:]
    if argv and argv[0] in COMMANDS:
        _canonical, command_cls = COMMANDS[argv[0]]
        # Dispatch directly to the selected Config so --config and
        # allow_subconfig_overrides=True use kwconf's full multipass loader.
        return command_cls.main(argv=argv[1:])
    return KwconfParityApp.main(argv=argv)


if __name__ == '__main__':
    main()
