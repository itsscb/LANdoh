import { Directory } from './directory';
import { SharedDirectory } from './source';


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
