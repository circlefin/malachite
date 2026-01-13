#!/usr/bin/env node

import { program } from 'commander';
import { runDeploy } from './commands/deploy';
import { runStatus } from './commands/status';

program
  .command('deploy')
  .description('Deploy the service')
  .action(runDeploy);

program
  .command('status')
  .description('Show service status')
  .action(runStatus);

program.parse(process.argv);
