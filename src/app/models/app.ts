import { Directory } from './directory';

export class App {
    address: string
    destination: string
    id: string
    nickname: string
    shared_directories: Directory[]
}

export   enum Severity {
    success = 'success',
    info = 'info',
    warn = 'warn',
    error = 'error'
  }

  export class FilePayload {
    id: string
    dir: string
    successful: string[]
    failed: string[]
  }