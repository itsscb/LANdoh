// import { confirm } from '@tauri-apps/api/dialog';
// import { homeDir } from '@tauri-apps/api/path';
import { MessageService, ConfirmEventType, ConfirmationService } from 'primeng/api';

import { Component, OnInit } from '@angular/core';

import { invoke } from '@tauri-apps/api';
import { open } from '@tauri-apps/api/dialog';
import { emit, listen } from '@tauri-apps/api/event'
import { appWindow } from '@tauri-apps/api/window';

import { Directory } from '../models/directory';
import { App, Severity } from '../models/app';

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
        this.toast(Severity.success, 'Seeding',selected.toString())
        this.app_state();
      }).catch(() => this.toast(Severity.error, 'Failed to seed', selected.toString()));
    }
  }

  async remove_shared_dir(name: string) {
    invoke('remove_shared_dir', {path: name, window: appWindow}).then(() => {
      this.toast(Severity.success, 'Unseeded',name)
      this.app_state();
    }).catch(() => this.toast(Severity.error, 'Failed to unseed',name));
  }


  listen_for() {
    if (!this.listening) {
      invoke('listen_for', { window: appWindow }).then(() => this.toast(Severity.info, 'Listening for shared directories')).catch(() => this.toast(Severity.error, 'Error starting Listener'));
      this.listening = true;
    }
  }

  serve() {
    if (!this.serving) {
      invoke('serve').then(() => this.toast(Severity.info, 'Serving shared directories')).catch(() => this.toast(Severity.error, 'Error starting Server'));
      this.serving = true;
    }
  }

  broadcast() {
    if(!this.broadcasting) {
      invoke("broadcast").then(() =>  this.toast(
        Severity.info,
        'Broadcasting shared directories',
      )).catch(() => this.toast(Severity.error, 'Error starting Broadcaster'));
      this.broadcasting = true;
    }
  }

  request_dir(id: string, name: string) {
    invoke("request_dir", { id: id, dir: name }).then(() =>  this.toast(
      Severity.info,
      'Leeching: ' + name + ' from ' + id,
    )).catch(() => this.toast(Severity.error, 'Error requesting Directory: '+name + ' from ' + id));
  }

 watch() {
    listen('sources', (event) => {
      this.new_sources = [...event.payload as Directory[]];
      this.app_state();
    })
  }

  toast(severity: Severity ,summary: string, detail?: string) {
    console.log(severity.toString());
    this.toastService.add({
      severity: severity.toString(),
      summary: summary,
      detail: detail
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
            this.toastService.add({ severity: 'info', summary: 'Unseeding', detail: '"'+name+'"' });
            this.remove_shared_dir(name);
        },
        reject: () => {
            this.toastService.add({ severity: 'error', summary: 'Still seeding', detail: '"'+name+'"'});
        }
    });
}

confirm_request_dir(event: Event,nick: string, id: string, name: string) {
   if(this.sources.some((s) => s.id == id && s.name == name) && this.new_sources != null && !this.new_sources.some((s) =>  s.id == id && s.name == name)) {
      this.toast(Severity.warn, 'Seed no longer available', 'Damn noobs!');
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
          this.toastService.add({ severity: 'info', summary: 'Leeching', detail: '"'+name+'" from "'+nick+'"' });
          this.request_dir(id, name);
      },
      reject: () => {
          this.toastService.add({ severity: 'error', summary: 'Canceled', detail: 'No one leeches anymore...'});
      }
  });
}

  apps: App[];
  app: App;

  dark: boolean;

  listening = false;
  serving = false;
  broadcasting = false;

  update_sources() {
    this.sources = [...this.new_sources];
    this.new_sources = null;
  }

  sources: Directory[];
  new_sources: Directory[];

  ngOnInit() {    
    this.app_state();
    this.listen_for();
    this.serve();
    this.broadcast();

    setInterval(async () => {
      let unlisten = await listen('sources', (event) => {
        this.new_sources = [...event.payload as Directory[]];
      })
      setInterval(() => unlisten(), 2000);
    }, 2000);
  }


}
