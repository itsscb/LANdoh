// import { confirm } from '@tauri-apps/api/dialog';
// import { homeDir } from '@tauri-apps/api/path';
import { MessageService, ConfirmEventType, ConfirmationService } from 'primeng/api';

import { Component, OnInit } from '@angular/core';

import { invoke } from '@tauri-apps/api';
import { open } from '@tauri-apps/api/dialog';
import { emit, listen } from '@tauri-apps/api/event'
import { appWindow } from '@tauri-apps/api/window';

import { TreeNode } from 'primeng/api';

import { Directory } from '../models/directory';
import { App, Severity, FilePayload } from '../models/app';

@Component({
  selector: 'app-home',
  templateUrl: './app.home.component.html',
  styleUrls: ['./app.home.component.css'],
  providers: [MessageService, ConfirmationService]
})
export class AppHomeComponent implements OnInit {
  constructor(private toastService: MessageService, private confirmationService: ConfirmationService) {}

  app_state() {
    invoke('app_state').then((s) => {
      this.apps = [s as App];
      this.app = structuredClone(s as App);
      console.log(this.app);
    })
  }

  open_dir(path: string) {
    invoke('open_dir', {path: path});
  }

  update_nickname(nick: string) {
    invoke('update_nickname', {nickname: nick, window: appWindow}).then(() => this.app_state());
  }

  async update_destination() {
    const selected = await open({
      multiple: false,
      directory: true,
    })
    invoke('update_destination', {destination: selected, window: appWindow}).then(() => this.app_state());
  }

  async add_shared_dir() {
    const selected = await open({
      multiple: false,
      directory: true,
    })

    let exists = false;
    this.app.shared_directories.filter((d) => {
      d.paths.filter((p) => {
        if (p == selected) {
          exists = true;
        }
    })});
    
    if (!exists) {
      invoke('add_shared_dir', {path: selected, window: appWindow}).then(() => {
        this.toast({severity:Severity.success,summary: 'Seeding',detail:selected.toString()})
        this.app_state();
      }).catch(() => this.toast({severity:Severity.error,summary: 'Failed to seed',detail: selected.toString()}));
    }
  }

  async remove_shared_dir(name: string) {
    invoke('remove_shared_dir', {path: name, window: appWindow}).then(() => {
      this.toast({severity: Severity.success,summary: 'Unseeded',detail:name})
      this.app_state();
    }).catch(() => this.toast({severity: Severity.error, summary:'Failed to unseed',detail: name}));
  }


  listen_for() {
    if (!this.listening) {
      invoke('listen_for', { window: appWindow }).then(() => this.toast({severity: Severity.info,summary: 'Listening for shared directories'})).catch(() => this.toast({severity: Severity.error, summary:'Error starting Listener'}));
      this.listening = true;
    }
  }

  serve() {
    if (!this.serving) {
      invoke('serve').then(() => this.toast({severity: Severity.info, summary:'Serving shared directories'})).catch(() => this.toast({severity: Severity.error, summary:'Error starting Server'}));
      this.serving = true;
    }
  }

  broadcast() {
    if(!this.broadcasting) {
      invoke("broadcast").then(() =>  this.toast(
        {
        severity: Severity.info,
        summary: 'Broadcasting shared directories',
        }
      )).catch(() => this.toast({severity: Severity.error, summary: 'Error starting Broadcaster'}));
      this.broadcasting = true;
    }
  }

  request_dir(id: string, name: string) {
    invoke("request_dir", { id: id, dir: name, window: appWindow}).catch(() => this.toast({severity: Severity.error, summary:'Error requesting Directory: '+name + ' from ' + id}));
  }

 watch() {
    listen('sources', (event) => {
      this.new_sources = [...event.payload as Directory[]];
      this.app_state();
    })
  }

  toast({severity, summary, detail = '', sticky = false} : {
    severity: Severity ,summary: string, detail?: string, sticky?: boolean
  }) {
    this.toastService.add({
      severity: severity.toString(),
      summary: summary,
      detail: detail,
      sticky: sticky
    });
  } 

  confirm_remove_dir(event: Event, name: string) {
    this.confirmationService.confirm({
        target: event.target as EventTarget,
        message: 'Are you sure you don\'t want to seed "'+name+'" anymore?',
        header: 'Unseeding Directory',
        icon: 'pi pi-exclamation-triangle',
        acceptIcon:"pi pi-trash",
        rejectIcon:"none",
        acceptLabel: "No one else shall have it but me!",
        rejectLabel: "Keep seeding",
        rejectButtonStyleClass:"p-button-text",
        acceptButtonStyleClass: "p-button-danger p-button text",
        accept: () => {
            this.toast({ severity: Severity.info, summary: 'Unseeding', detail: '"'+name+'"' });
            this.remove_shared_dir(name);
        },
        reject: () => {
            this.toast({ severity: Severity.error, summary: 'Still seeding', detail: '"'+name+'"'});
        }
    });
}

confirm_request_dir(event: Event,nick: string, id: string, name: string) {
   if(this.sources.some((s) => s.id == id && s.name == name) && this.new_sources != null && !this.new_sources.some((s) =>  s.id == id && s.name == name)) {
      this.toast({severity: Severity.warn, summary: 'Seed no longer available', detail: 'Damn noobs!'});
      return;
    }
  this.confirmationService.confirm({
      target: event.target as EventTarget,
      message: 'Leech "'+name+'" from "'+nick+'"?',
      header: 'Grab Directory',
      icon: 'pi pi-download',
      acceptIcon:"pi pi-check",
      rejectIcon:"none",
      acceptLabel: "Leech it already!",
      rejectLabel: "Nope",
      rejectButtonStyleClass:"p-button-text",
      acceptButtonStyleClass: "p-button-success p-button text",
      accept: () => {
          this.toast({ severity: Severity.info, summary: 'Leeching', detail: '"'+name+'" from "'+nick+'"' });
          this.request_dir(id, name);
      },
      reject: () => {
          this.toast({ severity: Severity.error, summary: 'Canceled', detail: 'No one leeches anymore...'});
      }
  });
}

  apps: App[];
  app: App;

  dark: boolean;

  listening = false;
  serving = false;
  broadcasting = false;

  downloadsSidebar = false;

  filePayloads: FilePayload[] = [];

  update_sources() {
    this.sources = [...this.new_sources];
    this.new_sources = null;
  }

  sources: Directory[];
  new_sources: Directory[];

  async ngOnInit() {    
    this.app_state();
    this.listen_for();
    this.serve();
    this.broadcast();

    // setInterval(async () => {
    //   let unlisten = await 
      await listen('sources', (event) => {
        this.new_sources = [...event.payload as Directory[]];
      })
    //   setInterval(() => unlisten(), 2000);
    // }, 2000);

    
    // setInterval(async () => {
      // let ul = await 
      await listen('files', (event) => {
        let p =event.payload as FilePayload;
        this.filePayloads.push(p);

        if(p.failed.length > 0) {
          if(p.successful.length < 1) {
            this.toast({severity: Severity.error,summary: 'Leech of "'+p.dir+'" failed', sticky: true})
          } else {
            this.toast({severity: Severity.error, summary: 'Leech of "'+p.dir+'" partially failed', sticky: true})
            this.toast({severity: Severity.success, summary: 'Leech of "'+p.dir+'" partially successful', sticky: true})
          }
        } else if (p.successful.length > 0){
          this.toast({severity: Severity.success, summary: 'Leech of "'+p.dir+'" successful', sticky: true})
        }

      });
    //   setInterval(() => ul(), 300);
    // }, 300);
  }


}
