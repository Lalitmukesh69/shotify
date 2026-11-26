const shouldRestartExistingDevServer = (_env) => false;
const killPort = (_port) => 0;
const applyViteWatcherEnv = (env) => ({ ...env });

const getViteDevSpawn = ({ nodeExe, viteBin, port }) => ({
  cmd: nodeExe,
  args: [viteBin, '--port', String(port), '--strictPort'],
});

module.exports = {
  shouldRestartExistingDevServer,
  killPort,
  applyViteWatcherEnv,
  getViteDevSpawn,
};
