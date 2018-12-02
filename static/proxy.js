
const stringify = JSON.stringify;
const stringifyPretty = (object) => stringify(object, null, 2);

const xRequest = new XMLHttpRequest();

const updatePre = (text) => {
    const preCollection = document.getElementsByTagName('pre');
    for (i = 0; i < preCollection.length; ++i) {
        preCollection[i].innerText = text;
    }
};

xRequest.onreadystatechange = function () {
    if (this.readyState == 4 && this.status == 200) {
        const responseObject = JSON.parse(xRequest.responseText);
        let preText = `Now: ${responseObject.now}\n\n`;
        preText += `${responseObject.method} ${responseObject.url}\n\n`;
        preText += `Response Status: ${responseObject.version} ${responseObject.status}\n\n`;
        preText += `Response Headers:\n${responseObject.headers}\n\n`;
        preText += responseObject.body;
        updatePre(preText);
    }
};

const requestData = (apiPath) => {
    xRequest.open('GET', apiPath, true);
    xRequest.setRequestHeader('Accept', 'application/json');
    xRequest.send();
};

const setTimer = (apiPath) => {
    const checkbox = document.getElementById('autoRefresh');

    setInterval(() => {
        if (checkbox.checked) {
            requestData(apiPath);
        }
    }, 1000);
};

const onload = (requestText, apiPath) => {
    let preText = `Now: ${new Date()}\n\n`;
    preText += `${requestText}\n\n`;
    preText += 'Response Status:\n\n';
    preText += 'Response Headers:';
    updatePre(preText);

    requestData(apiPath);

    setTimer(apiPath);
};
