#!/usr/bin/env python3
# fix relative path import
from core.pyvdm.utils import WorkSpace
import sys
from pathlib import Path
sys.path.append( Path(__file__).resolve().parent.as_posix() )
# normal import
import json, argparse
import tempfile, shutil
import ctypes
from functools import wraps
from pyvdm.interface import SRC_API
from pyvdm.core.utils import * #from utils import *

# set(CONFIG_DIR "$HOME/.vdm")
PLUGIN_DIRECTORY= Path('~/.vdm/plugins').expanduser()
REQUIRED_FIELDS = ['name', 'version', 'author', 'main', 'license']
OPTIONAL_FIELDS = ['description', 'keywords', 'capability', 'scripts']
OPTIONAL_SCRIPTS= ['pre-install', 'post-install', 'pre-uninstall', 'post-uninstall']
global args

class PluginWrapper():
    def __init__(self, entry):
        if entry.endswith('.py'):
            self.load_python(entry)
        elif entry.enswith('.so'):
            self.load_cdll(entry)
        else:
            raise Exception('Unsupported plugin entry.')
        pass

    def __getattribute__(self, name):
        if name.startswith('on'):
            try:
                _func = self.obj.__getattribute__(name)
                return _func
            except:
                print('%s is an illegal function name.'%name)
                return super().__getattribute__(name)  
        else:
            return super().__getattribute__(name)

    @staticmethod
    def wrap_call_on_string(func):
        @wraps(func)
        def _wrap(_string):
            return func( _string.encode() )
        return _wrap

    def load_python(self, entry):
        obj = None
        _module = __import__(entry)
        for obj in _module.__dict__.values():
            if isinstance(obj, SRC_API):
                break
        assert( isinstance(self.obj, SRC_API) )
        self.obj = obj
        pass

    def load_cdll(self, entry):
        obj = ctypes.cdll(entry)
        obj.onSave = self.wrap_call_on_string(obj.onSave)
        obj.onResume = self.wrap_call_on_string(obj.onResume)
        #obj.onTrigger
        self.obj = obj
        pass

    pass

class PluginManager:
    def __init__(self, root=''):
        if root:
            self.root = Path(root).resolve()
        else:
            self.root = PLUGIN_DIRECTORY
        self.root.mkdir(exist_ok=True, parents=True) #ensure root existing
        self.temp = Path( tempfile.gettempdir() )
        pass

    @staticmethod
    def test_config(config):
        # test required config fields
        for key in REQUIRED_FIELDS:
            if key not in config:
                return False # config: required field missing
        # test whether main entry is legal (*.py or *.so)
        if not (config['main'].endswith('.py') or config['main'].endswith('.so')):
            return False # config: illegal main entry
        # test whether main entry is provided
        _pre_built = Path('./release', config['main']).exists()
        _post_built= ('scripts' in config) and ('pre-install' in config['scripts'])
        if not (_pre_built or _post_built):
            return False # config: no existing main entry
        # all test pass
        return True

    def install(self, url):
        #TODO: if with online url, download as file in _path
        _path = Path(url).expanduser().resolve()
        # test whethere a file provided or not
        if not _path.is_file():
            return False #file_error
        # try to unpack the file to tmp_dir
        try:
            tmp_dir = self.temp / _path.stem
            shutil.unpack_archive( _path, POSIX(tmp_dir) )
        except:
            return False #file_error
        # try to test plugin integrity
        with WorkSpace(tmp_dir) as ws:
            try:
                _config = json.load('config.json')
                ret = self.test_config(_config)
                if ret!=True:
                    return ret
            except:
                return False #config file error
            try:
                _plugin = PluginWrapper(_config['main'])
            except Exception as e:
                return False #plugin loading error
            pass
        # move to root dir
        shutil.move( POSIX(tmp_dir), POSIX(self.root) )
        return True

    def uninstall(self, names):
        pass

    def list(self, name=[]):
        pass

    def run(self, name, function):
        pass

    pass

def execute(pm, command, args):
    assert( isinstance(pm, PluginManager) )
    if command=='install':
        pm.install(args.url)
    elif command=='uninstall':
        pm.uninstall(args.names)
    elif command=='list':
        pm.list(args.names)
    elif command=='run':
        pm.run(args.plugin_name, args.plugin_function)
    else:
        print('The command <{}> is not supported.'.format(command))
    pass

def init_subparsers(subparsers):
    p_install = subparsers.add_parser('install',
        help='install a new VDM plugin.')
    p_install.add_argument('url', metavar='plugin_file',
        help='the path to the plugin file in .zip format')
    #
    p_uninstall = subparsers.add_parser('uninstall',
        help='uninstall VDM plugins.')
    p_uninstall.add_argument('names', metavar='plugin_names', nargs='+',
        help='the plugin name(s) to uninstall.')
    #
    p_list = subparsers.add_parser('list',
        help='list information of installed VDM plugins.')
    p_list.add_argument('names', metavar='plugin_names', nargs='*',
        help='the specified plugin name(s) to list.')
    #
    p_run = subparsers.add_parser('run',
        help='run the function of an existing plugin.')
    p_run.add_argument('plugin_name',
        help='plugin name')
    p_run.add_argument('plugin_function',
        help='plugin function name')
    pass

if __name__ == '__main__':
    try:
        parser = argparse.ArgumentParser(
            description='VDM Plugin Manager.')
        subparsers = parser.add_subparsers(dest='command')
        init_subparsers(subparsers)
        #
        args = parser.parse_args()
        pm = PluginManager()
        execute(pm, args.command, args)
    except Exception as e:
        raise e#print(e)
    finally:
        pass#exit()
