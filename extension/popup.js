const status=document.querySelector('#status');
async function refresh(){try{const state=await chrome.runtime.sendMessage({action:'status'});status.textContent=state?.persistenceError?'Réglage non enregistré. Pour maintenir l’arrêt après redémarrage, désactive l’extension.':state?.connected?'Pont connecté — contrôle limité aux onglets attribués.':state?.verificationPassed?'Vérification locale enregistrée — activation en attente.':'Pont arrêté.';}catch{status.textContent='Pont indisponible.';}}
document.querySelector('#connect').addEventListener('click',async()=>{try{const response=await chrome.runtime.sendMessage({action:'connect'});if(response?.error)throw new Error('preference');status.textContent='Connexion…';}catch{status.textContent='Installation locale à vérifier.';}setTimeout(refresh,300);});
document.querySelector('#stop').addEventListener('click',async()=>{await chrome.runtime.sendMessage({action:'stop'});await refresh();});
void refresh();
setInterval(()=>void refresh(),1000);
