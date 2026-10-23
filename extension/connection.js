export class AutoConnection {
  #storage;#connect;#stop;#generation=0;#restoring;#writes=Promise.resolve();
  #enabled=false;#persistenceError=false;
  constructor({storage,connect,stop}){this.#storage=storage;this.#connect=connect;this.#stop=stop;}
  get enabled(){return this.#enabled;}
  get persistenceError(){return this.#persistenceError;}
  restore(){
    if(!this.#restoring)this.#restoring=this.#load();
    return this.#restoring;
  }
  async #load(){
    const generation=this.#generation;
    try{
      const saved=await this.#storage.get('autoConnectEnabled');
      if(generation!==this.#generation)return;
      this.#enabled=saved.autoConnectEnabled===undefined||saved.autoConnectEnabled===true;
      if(this.#enabled)this.#connect();
    }catch{if(generation===this.#generation){this.#enabled=false;this.#persistenceError=true;}}
  }
  #save(value){
    const writing=this.#writes.catch(()=>{}).then(()=>this.#storage.set({autoConnectEnabled:value}));
    this.#writes=writing;
    return writing;
  }
  async enable(){
    const generation=++this.#generation;
    try{await this.#save(true);}
    catch(error){if(generation===this.#generation)this.#persistenceError=true;throw error;}
    if(generation!==this.#generation)return;
    this.#persistenceError=false;this.#enabled=true;this.#connect();
  }
  async pause(){
    const generation=++this.#generation;
    this.#enabled=false;
    const stopping=this.#stop();
    let persisted=true;
    try{await this.#save(false);}
    catch{persisted=false;}
    if(generation===this.#generation)this.#persistenceError=!persisted;
    await stopping;
    return {stopped:true,persisted};
  }
}
