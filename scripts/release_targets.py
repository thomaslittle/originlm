"""The five native helper targets prepared by the release workflow.

This is the single source of truth for archive names, executable names and the
compatibility statements recorded in each target's provenance manifest. It does
not imply that every platform/architecture combination is supported.
"""

REPOSITORY = 'https://github.com/thomaslittle/originlm'

TARGETS = {
    'x86_64-unknown-linux-gnu': {
        'os': 'linux',
        'arch': 'x86_64',
        'platform': 'linux-x86_64',
        'archiveSuffix': '.tar.gz',
        'executable': 'originlm',
        'wheelPlatform': 'linux_x86_64',
        'support': 'Linux x86_64 with glibc; exact build-host baseline is recorded in the manifest',
    },
    'aarch64-unknown-linux-gnu': {
        'os': 'linux',
        'arch': 'aarch64',
        'platform': 'linux-aarch64',
        'archiveSuffix': '.tar.gz',
        'executable': 'originlm',
        'wheelPlatform': 'linux_aarch64',
        'support': 'Linux ARM64 with glibc; exact build-host baseline is recorded in the manifest',
    },
    'x86_64-apple-darwin': {
        'os': 'macos',
        'arch': 'x86_64',
        'platform': 'macos-x86_64',
        'archiveSuffix': '.tar.gz',
        'executable': 'originlm',
        'wheelPlatform': 'macosx_15_0_x86_64',
        'support': 'macOS 15 or newer on Intel; MACOSX_DEPLOYMENT_TARGET=15.0',
    },
    'aarch64-apple-darwin': {
        'os': 'macos',
        'arch': 'aarch64',
        'platform': 'macos-aarch64',
        'archiveSuffix': '.tar.gz',
        'executable': 'originlm',
        'wheelPlatform': 'macosx_15_0_arm64',
        'support': 'macOS 15 or newer on Apple silicon; MACOSX_DEPLOYMENT_TARGET=15.0',
    },
    'x86_64-pc-windows-msvc': {
        'os': 'windows',
        'arch': 'x86_64',
        'platform': 'windows-x86_64',
        'archiveSuffix': '.zip',
        'executable': 'originlm.exe',
        'wheelPlatform': 'win_amd64',
        'support': 'Windows x64; built and smoked on Windows Server 2025. Older Windows versions are not independently qualified.',
    },
}


def target_info(target):
    try:
        return TARGETS[target]
    except KeyError:
        raise ValueError(f'unsupported release target: {target}') from None


def archive_name(target):
    return f'originlm-{target}{target_info(target)["archiveSuffix"]}'


def helper_member(target):
    info = target_info(target)
    return f'helper-{target}/{info["executable"]}'
